use core::slice::SlicePattern;
use std::{mem::{size_of, self}, num::NonZeroU64};

use wgpu::{CommandEncoderDescriptor, ComputePassDescriptor};

use crate::{gpu::{GpuGlobalData, GpuAllocations}, chess::{board::{GpuBoard, self}, MAX_MOVES, Side, GameState, EvalScore}, buffers::{AllocToken, BufView}, misc::{ceil_div, SliceExtension}, shaders::{WORKGROUP_SIZE, ExpansionBindGroupMngr, ExpansionBuffers, FillMaxBindGroupMngr, FillMaxBuffers, EvalContractBindGroupMngr, EvalContractBuffers, ContractBindGroupMngr, ContractBuffers, FilterBindGroupMngr, FilterBuffers}};

/// A tree of chess positions that lives mainly on the gpu
pub struct GpuTree<'dev> {
    layers: Vec<GpuTreeLayer>,
    engine: &'dev GpuGlobalData,
    gpu_allocator: &'dev GpuAllocations,
}

impl<'dev> GpuTree<'dev> {
    pub fn new(engine: &'dev GpuGlobalData, allocator: &'dev GpuAllocations) -> Self {
        Self {
            layers: Vec::new(),
            engine,
            gpu_allocator: allocator,
        }
    }

    pub fn init_layer_from_state(&mut self, state: &GameState) {
        self.init_layer(&[board::convert(&state.get_board())], state.to_move);
    }

    pub fn init_layer(&mut self, boards: &[GpuBoard], to_move: Side) {
        let alloc = self.gpu_allocator.boards.allocate(boards.len() as u32);
        let data = bytemuck::cast_slice(boards);
        self.engine.queue.write_buffer(&alloc.buffer(&self.gpu_allocator.boards), alloc.start(), data);
        self.layers.push(GpuTreeLayer {
            num_boards: boards.len() as u32,
            to_move,
            board_buf: alloc,
            eval_buf: None,
        });
    }

    pub async fn filter_last_layer(&mut self, eval: EvalScore) {
        let last = self.layers.len()-1;
        self.filter(last, eval).await;
    }

    pub async fn expand_last_layer(&mut self) {
        let last = self.layers.last().unwrap();
        let mut new_layer = GpuTreeLayer {
            num_boards: 0,
            to_move: last.to_move.opposite(),
            board_buf: self.gpu_allocator.boards.allocate(last.num_boards * MAX_MOVES),
            eval_buf: None,
        };
        self.expand(last, &mut new_layer).await;
        self.layers.push(new_layer);
    }

    async fn expand(&self, from: &GpuTreeLayer, to: &mut GpuTreeLayer) {
        // Assert that the "to" allocation can always store the moves from the expansion
        assert!(to.board_buf.len() as u32 >= from.num_boards * MAX_MOVES);

        let bind = ExpansionBindGroupMngr::create(self.engine, &self.gpu_allocator, ExpansionBuffers {
            input: &from.board_buf,
            output: &to.board_buf,
        });
        
        self.engine.set_all_global_data(from.num_boards, from.to_move, 0, bind.1);
        let mut command_encoder = self.engine.device.create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass_encoder.set_pipeline(&self.engine.expand_shader.1);
        pass_encoder.set_bind_group(0, &bind.0, &[]);
        pass_encoder.dispatch_workgroups(ceil_div(from.num_boards, WORKGROUP_SIZE), 1, 1);
        drop(pass_encoder);
        command_encoder.copy_buffer_to_buffer(
            &self.engine.out_index,
            0, // Source offset
            &self.engine.out_index_staging,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        command_encoder.copy_buffer_to_buffer(
            &self.engine.just_zero,
            0, // Source offset
            &self.engine.out_index,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        self.engine.queue.submit([command_encoder.finish()]);

        self.engine.out_index_staging.slice(..).map_buffer(&self.engine.device, wgpu::MapMode::Read).await.unwrap();
        let out_index_view = self.engine.out_index_staging.slice(..).get_mapped_range();
        let output_size: u32 = u32::from_le(*bytemuck::from_bytes(&out_index_view.as_slice()));
        to.num_boards = output_size;
        drop(out_index_view);
        self.engine.out_index_staging.unmap();
    }

    async fn filter(&mut self, layer: usize, eval: EvalScore) {
        let layer = &mut self.layers[layer];
        let out_buf = self.gpu_allocator.boards.allocate(layer.num_boards);

        let bind = FilterBindGroupMngr::create(self.engine, &self.gpu_allocator, FilterBuffers {
            input: &layer.board_buf,
            output: &out_buf,
            eval: eval.raw(),
        });
        
        self.engine.set_all_global_data(layer.num_boards, layer.to_move, 0, bind.1);
        let mut command_encoder = self.engine.device.create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass_encoder.set_pipeline(&self.engine.filter_shader.1);
        pass_encoder.set_bind_group(0, &bind.0, &[]);
        pass_encoder.dispatch_workgroups(ceil_div(layer.num_boards, WORKGROUP_SIZE), 1, 1);
        drop(pass_encoder);
        command_encoder.copy_buffer_to_buffer(
            &self.engine.out_index,
            0, // Source offset
            &self.engine.out_index_staging,
            0, // Destination offset
            1 * size_of::<u32>() as u64,
        );
        command_encoder.clear_buffer(&self.engine.out_index, 0, None);
        self.engine.queue.submit([command_encoder.finish()]);

        self.engine.out_index_staging.slice(..).map_buffer(&self.engine.device, wgpu::MapMode::Read).await.unwrap();
        let out_index_view = self.engine.out_index_staging.slice(..).get_mapped_range();
        let output_size: u32 = u32::from_le(*bytemuck::from_bytes(&out_index_view.as_slice()));
        layer.num_boards = output_size;
        drop(out_index_view);
        self.engine.out_index_staging.unmap();

        let old_buf = mem::replace(&mut layer.board_buf, out_buf);
        self.gpu_allocator.boards.dealloc(old_buf);
    }


    pub async fn contract_eval(&mut self, layer: usize) {
        self.contract_generic(layer, true).await
    }

    pub async fn contract(&mut self, layer: usize) {
        self.contract_generic(layer, false).await
    }

    pub async fn contract_all(&mut self) {
        let last = self.last_layer().index;
        self.contract_eval(last).await;
        for i in (1..last).into_iter().rev() {
            self.contract(i).await;
        }
    }

    async fn contract_generic(&mut self, layer: usize, do_eval: bool) {
        let [parent_layer, child_layer] = self.layers.get_many_mut([layer - 1, layer]).unwrap();

        let to_move = parent_layer.to_move;
        let parent_num_boards = parent_layer.num_boards;
        let parent_eval = parent_layer.get_or_create_eval_buf(&self.gpu_allocator);
        let fill_max_bind = FillMaxBindGroupMngr::create(self.engine, &self.gpu_allocator, FillMaxBuffers {
            evals: parent_eval,
        });
        
        let generic_contract_bind = if do_eval {
            EvalContractBindGroupMngr::create(self.engine, &self.gpu_allocator, EvalContractBuffers {
                parent_evals_boards: parent_eval,
                child_boards: &child_layer.board_buf,
            })
        } else {
            ContractBindGroupMngr::create(self.engine, &self.gpu_allocator, ContractBuffers {
                parent_evals_boards: parent_eval,
                child_boards: &child_layer.board_buf,
                child_evals: child_layer.eval_buf.as_ref().expect("Can't contract if the children don't have evals")
            })
        };

        if to_move == Side::Black {
            // We should be able to optimize this and combine the data for these passes, so we don't need two command encoders
            let mut command_encoder2 = self.engine.device.create_command_encoder(&CommandEncoderDescriptor::default());
            let mut pass_encoder = command_encoder2.begin_compute_pass(&ComputePassDescriptor::default());
            self.engine.set_all_global_data(parent_num_boards, to_move, 0, fill_max_bind.1);
            pass_encoder.set_pipeline(&self.engine.fill_max_shader.1);
            pass_encoder.set_bind_group(0, &fill_max_bind.0, &[]);
            pass_encoder.dispatch_workgroups(ceil_div(parent_num_boards, WORKGROUP_SIZE), 1, 1);
            drop(pass_encoder);
            self.engine.queue.submit([command_encoder2.finish()]);
        }
        let mut command_encoder = self.engine.device.create_command_encoder(&CommandEncoderDescriptor::default());
        if to_move == Side::White {
            match NonZeroU64::try_from(parent_eval.byte_len()) {
                Ok(size) => {
                    command_encoder.clear_buffer(&parent_eval.buffer(&self.gpu_allocator.evals), parent_eval.start(), Some(size));
                },
                Err(_) => {},
            }
        }
        let mut pass_encoder = command_encoder.begin_compute_pass(&ComputePassDescriptor::default());
        self.engine.set_all_global_data(child_layer.num_boards, to_move, 0, generic_contract_bind.1);
        if do_eval {
            pass_encoder.set_pipeline(&self.engine.eval_contract_shader.1);
        } else {
            pass_encoder.set_pipeline(&self.engine.contract_shader.1);
        }
        pass_encoder.set_bind_group(0, &generic_contract_bind.0, &[]);
        pass_encoder.dispatch_workgroups(ceil_div(child_layer.num_boards, WORKGROUP_SIZE), 1, 1);
        drop(pass_encoder);
        self.engine.queue.submit([command_encoder.finish()]);
    }

    pub async fn view_boards_last(&self) -> BufView<'_, GpuBoard> {
        self.view_boards(self.layers.len()-1).await
    }

    async fn view_boards(&self, layer: usize) -> BufView<'_, GpuBoard> {
        let layer = &self.layers[layer];
        let view = self.gpu_allocator.boards.view(&self.engine.queue, &layer.board_buf, 0..layer.num_boards).await.unwrap();

        return view;
    }

    pub async fn view_evals(&self, layer: usize) -> BufView<'_, EvalScore> {
        let layer = &self.layers[layer];
        let view = self.gpu_allocator.evals.view(&self.engine.queue, &layer.eval_buf.as_ref().unwrap(), 0..layer.num_boards).await.unwrap();

        return view;
    }

    pub async fn debug_evals(&self, layer: usize) {
        let e = self.view_evals(layer).await;
        let min = e.cast_t().iter().min().unwrap();
        let max = e.cast_t().iter().max().unwrap();
        println!("Eval buffer for layer {} contains values from {}..{} and is sized {}", layer, min.raw(), max.raw(), e.cast_t().len())
    }

    pub fn layer(&self, layer: usize) -> LayerRef<'_> {
        LayerRef { index: layer, tree: self }
    }

    pub fn last_layer(&self) -> LayerRef<'_> {
        LayerRef { index: self.layers.len()-1, tree: self }
    }
}

struct GpuTreeLayer {
    num_boards: u32,
    to_move: Side,
    board_buf: AllocToken<GpuBoard>,
    eval_buf: Option<AllocToken<EvalScore>>,
}

impl GpuTreeLayer {
    pub fn get_or_create_eval_buf(&mut self, alloc: &GpuAllocations) -> &AllocToken<EvalScore> {
        self.eval_buf.get_or_insert_with(|| {
            alloc.evals.allocate(self.num_boards)
        })
    }
}

impl Drop for GpuTree<'_> {
    fn drop(&mut self) {
        self.layers.drain(..).for_each(|layer| {
            self.gpu_allocator.boards.dealloc(layer.board_buf);
            if let Some(eval_buf) = layer.eval_buf {
                self.gpu_allocator.evals.dealloc(eval_buf);
            }
        });
    }
}

pub struct LayerRef<'a> {
    index: usize,
    tree: &'a GpuTree<'a>,
}

impl<'a> LayerRef<'a> {
    pub fn size(&self) -> u32 {
        self.inner().num_boards
    }

    pub fn depth(&mut self) -> usize {
        self.index
    }

    fn inner(&self) -> &GpuTreeLayer {
        &self.tree.layers[self.index]
    }
}

pub struct LayerRefMut<'a> {
    index: usize,
    tree: &'a mut GpuTree<'a>,
}

impl<'a> LayerRefMut<'a> {
    pub fn size(&self) -> u32 {
        self.inner().num_boards
    }

    pub fn depth(&mut self) -> usize {
        self.index
    }

    fn inner(&self) -> &GpuTreeLayer {
        &self.tree.layers[self.index]
    }
}