use crate::buffer::Buffer;
use ash::{version::DeviceV1_0, vk};

#[derive(Debug, Clone)]
struct InvalidHandle;
impl std::fmt::Display for InvalidHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid handle")
    }
}
impl std::error::Error for InvalidHandle {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub struct Model<V, I> {
    vertex_data: Vec<V>,
    index_data: Vec<u32>,
    handle_to_index: std::collections::HashMap<usize, usize>,
    handles: Vec<usize>,
    instances: Vec<I>,
    first_invisible: usize,
    next_handle: usize,
    pub vertex_buffer: Option<Buffer>,
    pub index_buffer: Option<Buffer>,
    pub instance_buffer: Option<Buffer>,
}

#[allow(dead_code)]
impl<V, I> Model<V, I> {
    fn get(&self, handle: usize) -> Option<&I> {
        if let Some(&index) = self.handle_to_index.get(&handle) {
            self.instances.get(index)
        } else {
            None
        }
    }
    fn get_mut(&mut self, handle: usize) -> Option<&mut I> {
        if let Some(&index) = self.handle_to_index.get(&handle) {
            self.instances.get_mut(index)
        } else {
            None
        }
    }
    fn swap_by_handle(&mut self, handle1: usize, handle2: usize) -> Result<(), InvalidHandle> {
        if handle1 == handle2 {
            return Ok(());
        }
        if let (Some(&index1), Some(&index2)) = (
            self.handle_to_index.get(&handle1),
            self.handle_to_index.get(&handle2),
        ) {
            self.handles.swap(index1, index2);
            self.instances.swap(index1, index2);
            self.handle_to_index.insert(index1, handle2);
            self.handle_to_index.insert(index2, handle1);
            Ok(())
        } else {
            Err(InvalidHandle)
        }
    }
    fn swap_by_index(&mut self, index1: usize, index2: usize) {
        if index1 == index2 {
            return;
        }
        let handle1 = self.handles[index1];
        let handle2 = self.handles[index2];
        self.handles.swap(index1, index2);
        self.instances.swap(index1, handle2);
        self.handle_to_index.insert(index1, handle2);
        self.handle_to_index.insert(index2, handle1);
    }
    fn is_visible(&self, handle: usize) -> Result<bool, InvalidHandle> {
        if let Some(index) = self.handle_to_index.get(&handle) {
            Ok(index < &self.first_invisible)
        } else {
            Err(InvalidHandle)
        }
    }
    fn make_visible(&mut self, handle: usize) -> Result<(), InvalidHandle> {
        // if already visible do nothing
        if let Some(&index) = self.handle_to_index.get(&handle) {
            if index < self.first_invisible {
                return Ok(());
            }
            // else move position first_invisible and increase value of first_invisible
            self.swap_by_index(index, self.first_invisible);
            self.first_invisible += 1;
            Ok(())
        } else {
            Err(InvalidHandle)
        }
    }
    fn make_invisible(&mut self, handle: usize) -> Result<(), InvalidHandle> {
        // if already invisible do nothing
        if let Some(&index) = self.handle_to_index.get(&handle) {
            if index >= self.first_invisible {
                return Ok(());
            }
            // else move position before first_invisible and decrease value of first_invisible
            self.swap_by_index(index, self.first_invisible - 1);
            self.first_invisible -= 1;
            Ok(())
        } else {
            Err(InvalidHandle)
        }
    }
    fn insert(&mut self, element: I) -> usize {
        let handle = self.next_handle;
        self.next_handle += 1;
        let index = self.instances.len();
        self.instances.push(element);
        self.handles.push(handle);
        self.handle_to_index.insert(handle, index);
        handle
    }
    pub fn insert_visibly(&mut self, element: I) -> usize {
        let new_handle = self.insert(element);
        self.make_visible(new_handle).ok(); //cant go wrong, see previous line
        new_handle
    }
    fn remove(&mut self, handle: usize) -> Result<I, InvalidHandle> {
        if let Some(&index) = self.handle_to_index.get(&handle) {
            if index < self.first_invisible {
                self.swap_by_index(index, self.first_invisible - 1);
                self.first_invisible -= 1;
            }
            self.swap_by_index(self.first_invisible, self.instances.len() - 1);
            self.handles.pop();
            self.handle_to_index.remove(&handle);
            //must be Some(), otherwise we couldnt have found and index
            Ok(self.instances.pop().unwrap())
        } else {
            Err(InvalidHandle)
        }
    }
    pub fn update_vertex_buffer(
        &mut self,
        allocator: &vk_mem::Allocator,
    ) -> Result<(), vk_mem::error::Error> {
        if let Some(buffer) = &mut self.vertex_buffer {
            buffer.fill(allocator, &self.vertex_data)?;
            Ok(())
        } else {
            let bytes = (self.vertex_data.len() * std::mem::size_of::<V>()) as u64;
            let mut buffer = Buffer::new(
                &allocator,
                bytes,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk_mem::MemoryUsage::CpuToGpu,
            )?;
            buffer.fill(allocator, &self.vertex_data)?;
            self.vertex_buffer = Some(buffer);
            Ok(())
        }
    }
    pub fn update_index_buffer(
        &mut self,
        allocator: &vk_mem::Allocator,
    ) -> Result<(), vk_mem::error::Error> {
        if let Some(buffer) = &mut self.index_buffer {
            buffer.fill(allocator, &self.index_data)?;
            Ok(())
        } else {
            let bytes = (self.index_data.len() * std::mem::size_of::<u32>()) as u64;
            let mut buffer = Buffer::new(
                &allocator,
                bytes,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk_mem::MemoryUsage::CpuToGpu,
            )?;
            buffer.fill(allocator, &self.index_data)?;
            self.index_buffer = Some(buffer);
            Ok(())
        }
    }
    pub fn update_instance_buffer(
        &mut self,
        allocator: &vk_mem::Allocator,
    ) -> Result<(), vk_mem::error::Error> {
        if let Some(buffer) = &mut self.instance_buffer {
            buffer.fill(allocator, &self.instances[0..self.first_invisible])?;
            Ok(())
        } else {
            let bytes = (self.first_invisible * std::mem::size_of::<I>()) as u64;
            let mut buffer = Buffer::new(
                &allocator,
                bytes,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk_mem::MemoryUsage::CpuToGpu,
            )?;
            buffer.fill(allocator, &self.instances[0..self.first_invisible])?;
            self.instance_buffer = Some(buffer);
            Ok(())
        }
    }
    pub fn draw(&self, logical_device: &ash::Device, command_buffer: vk::CommandBuffer) {
        if let Some(vertex_buffer) = &self.vertex_buffer {
            if let Some(index_buffer) = &self.index_buffer {
                if let Some(instance_buffer) = &self.instance_buffer {
                    if self.first_invisible > 0 {
                        unsafe {
                            logical_device.cmd_bind_vertex_buffers(
                                command_buffer,
                                0,
                                &[vertex_buffer.buffer],
                                &[0],
                            );
                            logical_device.cmd_bind_vertex_buffers(
                                command_buffer,
                                1,
                                &[instance_buffer.buffer],
                                &[0],
                            );
                            logical_device.cmd_bind_index_buffer(
                                command_buffer,
                                index_buffer.buffer,
                                0,
                                vk::IndexType::UINT32,
                            );
                            logical_device.cmd_draw_indexed(
                                command_buffer,
                                self.index_data.len() as u32,
                                self.first_invisible as u32,
                                0,
                                0,
                                0,
                            );
                        }
                    }
                }
            }
        }
    }
}

impl Model<[f32; 3], InstanceData> {
    pub fn icosahedron() -> Model<[f32; 3], InstanceData> {
        let phi = (1.0 + 5.0_f32.sqrt()) / 2.0;
        let darkgreen_front_top = [phi, -1.0, 0.0]; // 0
        let darkgreen_front_bottom = [phi, 1.0, 0.0]; // 1
        let darkgreen_back_top = [-phi, -1.0, 0.0]; // 2
        let darkgreen_back_bottom = [-phi, 1.0, 0.0]; // 3
        let lightgreen_front_right = [1.0, 0.0, -phi]; // 4
        let lightgreen_front_left = [-1.0, 0.0, -phi]; // 5
        let lightgreen_back_right = [1.0, 0.0, phi]; // 6
        let lightgreen_back_left = [-1.0, 0.0, phi]; // 7
        let purple_top_left = [0.0, -phi, -1.0]; // 8
        let purple_top_right = [0.0, -phi, 1.0]; // 9
        let purple_bottom_left = [0.0, phi, -1.0]; // 10
        let purple_bottom_right = [0.0, phi, 1.0]; // 11
        Model {
            vertex_data: vec![
                darkgreen_front_top,
                darkgreen_front_bottom,
                darkgreen_back_top,
                darkgreen_back_bottom,
                lightgreen_front_right,
                lightgreen_front_left,
                lightgreen_back_right,
                lightgreen_back_left,
                purple_top_left,
                purple_top_right,
                purple_bottom_left,
                purple_bottom_right,
            ],
            index_data: vec![
                0, 9, 8, //
                0, 8, 4, //
                0, 4, 1, //
                0, 1, 6, //
                0, 6, 9, //
                8, 9, 2, //
                8, 2, 5, //
                8, 5, 4, //
                4, 5, 10, //
                4, 10, 1, //
                1, 10, 11, //
                1, 11, 6, //
                2, 3, 5, //
                2, 7, 3, //
                2, 9, 7, //
                5, 3, 10, //
                3, 11, 10, //
                3, 7, 11, //
                6, 7, 9, //
                6, 11, 7, //
            ],
            handle_to_index: std::collections::HashMap::new(),
            handles: Vec::new(),
            instances: Vec::new(),
            first_invisible: 0,
            next_handle: 0,
            vertex_buffer: None,
            index_buffer: None,
            instance_buffer: None,
        }
    }

    pub fn sphere(refinements: u32) -> Model<[f32; 3], InstanceData> {
        let mut model = Model::icosahedron();
        for _ in 0..refinements {
            model.refine();
        }
        for v in &mut model.vertex_data {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            *v = [v[0] / l, v[1] / l, v[2] / l];
        }
        model
    }

    #[allow(dead_code)]
    pub fn cube() -> Model<[f32; 3], InstanceData> {
        let lbf = [-1.0, 1.0, -1.0]; //lbf: left-bottom-front
        let lbb = [-1.0, 1.0, 1.0];
        let ltf = [-1.0, -1.0, -1.0];
        let ltb = [-1.0, -1.0, 1.0];
        let rbf = [1.0, 1.0, -1.0];
        let rbb = [1.0, 1.0, 1.0];
        let rtf = [1.0, -1.0, -1.0];
        let rtb = [1.0, -1.0, 1.0];
        Model {
            vertex_data: vec![lbf, lbb, ltf, ltb, rbf, rbb, rtf, rtb],
            index_data: vec![
                0, 1, 5, 0, 5, 4, //bottom
                2, 7, 3, 2, 6, 7, //top
                0, 6, 2, 0, 4, 6, //front
                1, 3, 7, 1, 7, 5, //back
                0, 2, 1, 1, 2, 3, //left
                4, 5, 6, 5, 7, 6, //right
            ],
            handle_to_index: std::collections::HashMap::new(),
            handles: Vec::new(),
            instances: Vec::new(),
            first_invisible: 0,
            next_handle: 0,
            vertex_buffer: None,
            index_buffer: None,
            instance_buffer: None,
        }
    }

    pub fn refine(&mut self) {
        let mut new_indicies = vec![];
        let mut midpoints = std::collections::HashMap::<(u32, u32), u32>::new();
        for triangle in self.index_data.chunks(3) {
            let a = triangle[0];
            let b = triangle[1];
            let c = triangle[2];
            let vertex_a = self.vertex_data[a as usize];
            let vertex_b = self.vertex_data[b as usize];
            let vertex_c = self.vertex_data[c as usize];
            let mab = if let Some(ab) = midpoints.get(&(a, b)) {
                *ab
            } else {
                let vertex_ab = [
                    0.5 * (vertex_a[0] + vertex_b[0]),
                    0.5 * (vertex_a[1] + vertex_b[1]),
                    0.5 * (vertex_a[2] + vertex_b[2]),
                ];
                let mab = self.vertex_data.len() as u32;
                self.vertex_data.push(vertex_ab);
                midpoints.insert((a, b), mab);
                midpoints.insert((b, a), mab);
                mab
            };
            let mbc = if let Some(bc) = midpoints.get(&(b, c)) {
                *bc
            } else {
                let vertex_bc = [
                    0.5 * (vertex_b[0] + vertex_c[0]),
                    0.5 * (vertex_b[1] + vertex_c[1]),
                    0.5 * (vertex_b[2] + vertex_c[2]),
                ];
                let mbc = self.vertex_data.len() as u32;
                self.vertex_data.push(vertex_bc);
                midpoints.insert((b, c), mbc);
                midpoints.insert((c, b), mbc);
                mbc
            };
            let mca = if let Some(ca) = midpoints.get(&(c, a)) {
                *ca
            } else {
                let vertex_ca = [
                    0.5 * (vertex_c[0] + vertex_a[0]),
                    0.5 * (vertex_c[1] + vertex_a[1]),
                    0.5 * (vertex_c[2] + vertex_a[2]),
                ];
                let mca = self.vertex_data.len() as u32;
                self.vertex_data.push(vertex_ca);
                midpoints.insert((c, a), mca);
                midpoints.insert((a, c), mca);
                mca
            };
            new_indicies.extend_from_slice(&[mca, a, mab, mab, b, mbc, mbc, c, mca, mab, mbc, mca]);
        }
        self.index_data = new_indicies;
    }
}

#[repr(C)]
pub struct InstanceData {
    pub model_matrix: [[f32; 4]; 4],
    pub color: [f32; 3],
}
