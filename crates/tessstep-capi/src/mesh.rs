use crate::{
    TS_INVALID_ARGUMENT, TS_INVALID_MESH, TS_OK, TS_RESOURCE_LIMIT, boundary, clear, initialize,
};
use std::{ptr, sync::Arc};
use tessstep_mesh::{Limits, Mesh};
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsMeshOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_vertices: u64,
    pub max_triangles: u64,
    pub max_work: u64,
    pub require_solid: u32,
    pub reserved: u32,
}
impl Default for TsMeshOptions {
    fn default() -> Self {
        let l = Limits::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_vertices: l.max_vertices as u64,
            max_triangles: l.max_triangles as u64,
            max_work: l.max_work as u64,
            require_solid: 0,
            reserved: 0,
        }
    }
}
/// Private scalar storage implementing the C buffer protocol; no kernel layouts exposed.
#[derive(Debug)]
pub struct TsMesh {
    pub(crate) kernel: Arc<Mesh>,
    positions: Vec<f64>,
    triangles: Vec<u32>,
    normals: Vec<f64>,
    uvs: Vec<f64>,
    faces: Vec<u64>,
    statistics: tessstep_mesh::Statistics,
}
impl From<Mesh> for TsMesh {
    fn from(mesh: Mesh) -> Self {
        let d = mesh.data();
        Self {
            positions: d.positions.iter().flatten().copied().collect(),
            triangles: d.triangles.iter().flatten().copied().collect(),
            normals: d.normals.iter().flatten().flatten().copied().collect(),
            uvs: d.uvs.iter().flatten().flatten().copied().collect(),
            faces: d.face_ids.clone(),
            statistics: mesh.statistics(),
            kernel: Arc::new(mesh),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsMeshView {
    pub positions: *const f64,
    pub vertex_count: usize,
    pub triangles: *const u32,
    pub triangle_count: usize,
    pub normals: *const f64,
    pub uvs: *const f64,
    pub face_ids: *const u64,
    pub boundary_edges: u64,
    pub components: u64,
    pub signed_volume: f64,
}
impl Default for TsMeshView {
    fn default() -> Self {
        Self {
            positions: ptr::null(),
            vertex_count: 0,
            triangles: ptr::null(),
            triangle_count: 0,
            normals: ptr::null(),
            uvs: ptr::null(),
            face_ids: ptr::null(),
            boundary_edges: 0,
            components: 0,
            signed_volume: 0.,
        }
    }
}
/// # Safety
/// Follow the output storage contract in tessstep.h.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_mesh_options_init(out: *mut TsMeshOptions) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is valid writable C options storage.
        if unsafe { clear(out) } {
            TS_OK
        } else {
            TS_INVALID_ARGUMENT
        }
    })
}
/// # Safety
/// Follow tessstep.h readable input counts and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_mesh_create(
    positions: *const f64,
    vertex_count: usize,
    triangles: *const u32,
    triangle_count: usize,
    options: *const TsMeshOptions,
    out: *mut *const TsMesh,
) -> u32 {
    // SAFETY: out is a required writable, aligned pointer slot when non-NULL.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: non-NULL options points to the complete C-defined options record.
        let o = if options.is_null() {
            TsMeshOptions::default()
        } else {
            unsafe { *options }
        };
        let Limits {
            max_vertices,
            max_triangles,
            max_work,
        } = match o.limits() {
            Ok(limits) => limits,
            Err(status) => return status,
        };
        if vertex_count > max_vertices || triangle_count > max_triangles {
            return TS_RESOURCE_LIMIT;
        }
        if (positions.is_null() && vertex_count != 0)
            || (triangles.is_null() && triangle_count != 0)
            || vertex_count > isize::MAX as usize / (3 * size_of::<f64>())
            || triangle_count > isize::MAX as usize / (3 * size_of::<u32>())
        {
            return TS_INVALID_ARGUMENT;
        }
        if vertex_count.saturating_add(triangle_count) > max_work {
            return TS_RESOURCE_LIMIT;
        }
        // SAFETY: caller guarantees aligned readable arrays of the documented lengths.
        // Empty arrays are handled separately since Rust slices require non-NULL pointers.
        let p = if vertex_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(positions, 3 * vertex_count) }
        };
        let t = if triangle_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(triangles, 3 * triangle_count) }
        };
        let result = Mesh::from_triangles(
            p.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect(),
            t.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect(),
            Limits {
                max_vertices,
                max_triangles,
                max_work,
            },
        );
        let mesh = match result {
            Ok(m) => m,
            Err(tessstep_mesh::Error::ResourceLimit) => return TS_RESOURCE_LIMIT,
            Err(_) => return TS_INVALID_MESH,
        };
        if o.require_solid != 0 && mesh.require_solid().is_err() {
            return TS_INVALID_MESH;
        }
        let handle = Arc::into_raw(Arc::new(TsMesh::from(mesh)));
        // SAFETY: valid output slot; publish ownership only after all fallible work.
        unsafe { out.write(handle) };
        TS_OK
    })
}
/// # Safety
/// The non-NULL mesh must be live for the entire call; each retain needs release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_mesh_retain(mesh: *const TsMesh) -> u32 {
    boundary(|| {
        if mesh.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: mesh was returned by this library and its strong count remains positive.
        unsafe { Arc::increment_strong_count(mesh) };
        TS_OK
    })
}
/// # Safety
/// Releases exactly one acquisition; NULL is permitted. Follow tessstep.h lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_mesh_release(mesh: *const TsMesh) {
    if !mesh.is_null() {
        // SAFETY: caller transfers one live strong reference from create/retain.
        drop(unsafe { Arc::from_raw(mesh) });
    }
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_mesh_get_view(mesh: *const TsMesh, out: *mut TsMeshView) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a valid writable C view record.
        if !unsafe { clear(out) } || mesh.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps a live mesh reference throughout this query and view use.
        let m = unsafe { &*mesh };
        let view = TsMeshView {
            positions: m.positions.as_ptr(),
            vertex_count: m.positions.len() / 3,
            triangles: m.triangles.as_ptr(),
            triangle_count: m.triangles.len() / 3,
            normals: m.normals.as_ptr(),
            uvs: m.uvs.as_ptr(),
            face_ids: m.faces.as_ptr(),
            boundary_edges: m.statistics.boundary_edges as u64,
            components: m.statistics.components as u64,
            signed_volume: m.statistics.signed_volume,
        };
        // SAFETY: initialized writable output; scalar/pointer C fields only.
        unsafe { out.write(view) };
        TS_OK
    })
}

impl TsMeshOptions {
    pub(crate) fn limits(self) -> Result<Limits, u32> {
        if self.struct_size != size_of::<Self>() as u32
            || self.abi_version != 1
            || self.require_solid > 1
            || self.reserved != 0
        {
            return Err(TS_INVALID_ARGUMENT);
        }
        let count = |v| usize::try_from(v).map_err(|_| TS_INVALID_ARGUMENT);
        Ok(Limits {
            max_vertices: count(self.max_vertices)?,
            max_triangles: count(self.max_triangles)?,
            max_work: count(self.max_work)?,
        })
    }
}
