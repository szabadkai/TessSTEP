use crate::{
    TS_INVALID_ARGUMENT, TS_INVALID_MESH, TS_INVALID_SCENE, TS_NOT_FOUND, TS_OK, TS_RESOURCE_LIMIT,
    TsMesh, TsMeshOptions, boundary, clear, initialize,
};
use std::{collections::BTreeMap, ptr, sync::Arc};
use tessstep_mesh::scene::{self, AssetId, InstanceId, Scene};
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsSceneOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_assets: u64,
    pub max_instances: u64,
    pub max_depth: u64,
    pub max_work: u64,
}
impl Default for TsSceneOptions {
    fn default() -> Self {
        let l = scene::Limits::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_assets: l.max_assets as u64,
            max_instances: l.max_instances as u64,
            max_depth: l.max_depth as u64,
            max_work: l.max_work as u64,
        }
    }
}
impl TsSceneOptions {
    fn limits(self) -> Result<scene::Limits, u32> {
        if self.struct_size != size_of::<Self>() as u32 || self.abi_version != 1 {
            return Err(TS_INVALID_ARGUMENT);
        }
        let count = |v| usize::try_from(v).map_err(|_| TS_INVALID_ARGUMENT);
        Ok(scene::Limits {
            max_assets: count(self.max_assets)?,
            max_instances: count(self.max_instances)?,
            max_depth: count(self.max_depth)?,
            max_work: count(self.max_work)?,
            ..Default::default()
        })
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsSceneAsset {
    pub id: u64,
    pub mesh: *const TsMesh,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSceneInstance {
    pub id: u64,
    pub parent_id: u64,
    pub asset_id: u64,
    pub linear: [f64; 9],
    pub translation: [f64; 3],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSceneInstanceInfo {
    pub source: TsSceneInstance,
    pub world_linear: [f64; 9],
    pub world_translation: [f64; 3],
    pub depth: u64,
    pub mirrored: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsSceneInfo {
    pub asset_count: usize,
    pub instance_count: usize,
}
#[derive(Debug)]
pub struct TsScene {
    pub(crate) scene: Arc<Scene>,
    // Retain the original C scalar storage too, so acquiring an asset never copies buffers.
    assets: BTreeMap<AssetId, Arc<TsMesh>>,
}
fn status(error: scene::Error) -> u32 {
    match error {
        scene::Error::ResourceLimit => TS_RESOURCE_LIMIT,
        scene::Error::MissingInstance(_)
        | scene::Error::MissingAsset(_)
        | scene::Error::NoAsset(_) => TS_NOT_FOUND,
        scene::Error::Mesh(_) => TS_INVALID_MESH,
        _ => TS_INVALID_SCENE,
    }
}
fn input(instance: scene::Instance) -> TsSceneInstance {
    let linear = instance.local_transform.linear();
    TsSceneInstance {
        id: instance.id.0,
        parent_id: instance.parent.map_or(0, |id| id.0),
        asset_id: instance.asset.map_or(0, |id| id.0),
        linear: std::array::from_fn(|i| linear[i / 3][i % 3]),
        translation: instance.local_transform.translation(),
    }
}
/// # Safety
/// Follow tessstep.h writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_options_init(out: *mut TsSceneOptions) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out points to a writable C options record.
        if unsafe { clear(out) } {
            TS_OK
        } else {
            TS_INVALID_ARGUMENT
        }
    })
}
/// # Safety
/// Follow tessstep.h readable arrays, live mesh handles and disjoint outputs contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_create(
    assets: *const TsSceneAsset,
    asset_count: usize,
    instances: *const TsSceneInstance,
    instance_count: usize,
    options: *const TsSceneOptions,
    out: *mut *const TsScene,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        // SAFETY: non-NULL options points to a complete readable record.
        let o = if options.is_null() {
            TsSceneOptions::default()
        } else {
            unsafe { *options }
        };
        let limits = match o.limits() {
            Ok(l) => l,
            Err(s) => return s,
        };
        if asset_count > limits.max_assets
            || instance_count > limits.max_instances
            || asset_count
                .checked_add(instance_count)
                .is_none_or(|n| n > limits.max_work)
        {
            return TS_RESOURCE_LIMIT;
        }
        if (assets.is_null() && asset_count != 0)
            || (instances.is_null() && instance_count != 0)
            || asset_count > isize::MAX as usize / size_of::<TsSceneAsset>()
            || instance_count > isize::MAX as usize / size_of::<TsSceneInstance>()
        {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: input arrays are readable and aligned for their documented record counts.
        let assets = if asset_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(assets, asset_count) }
        };
        // SAFETY: as above, empty arrays avoid creating a slice from NULL.
        let instances = if instance_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(instances, instance_count) }
        };
        let mut retained = BTreeMap::new();
        let mut kernel_assets = Vec::with_capacity(asset_count);
        for asset in assets {
            if asset.mesh.is_null() {
                return TS_INVALID_ARGUMENT;
            }
            if asset.id == 0 || retained.contains_key(&AssetId(asset.id)) {
                return TS_INVALID_SCENE;
            }
            // SAFETY: caller keeps the library-created mesh live during this call.
            // Increment before from_raw so this temporary owns its independent acquisition.
            let mesh = unsafe {
                Arc::increment_strong_count(asset.mesh);
                Arc::from_raw(asset.mesh)
            };
            kernel_assets.push(scene::Asset {
                id: AssetId(asset.id),
                mesh: mesh.kernel.clone(),
            });
            retained.insert(AssetId(asset.id), mesh);
        }
        let mut nodes = Vec::with_capacity(instance_count);
        for instance in instances {
            let t = match scene::Transform::new(
                std::array::from_fn(|i| std::array::from_fn(|j| instance.linear[i * 3 + j])),
                instance.translation,
            ) {
                Ok(t) => t,
                Err(_) => return TS_INVALID_SCENE,
            };
            nodes.push(scene::Instance {
                id: InstanceId(instance.id),
                parent: (instance.parent_id != 0).then_some(InstanceId(instance.parent_id)),
                asset: (instance.asset_id != 0).then_some(AssetId(instance.asset_id)),
                local_transform: t,
            });
        }
        let limits = scene::Limits {
            max_work: limits.max_work - asset_count - instance_count,
            ..limits
        };
        let scene = match Scene::new(kernel_assets, nodes, limits) {
            Ok(s) => s,
            Err(scene::Error::ResourceLimit) => return TS_RESOURCE_LIMIT,
            Err(_) => return TS_INVALID_SCENE,
        };
        let handle = Arc::into_raw(Arc::new(TsScene {
            scene: Arc::new(scene),
            assets: retained,
        }));
        // SAFETY: valid output slot, publish only after successful validation.
        unsafe { out.write(handle) };
        TS_OK
    })
}
/// # Safety
/// Keep a live scene acquisition for this call; pair each retain with release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_retain(scene: *const TsScene) -> u32 {
    boundary(|| {
        if scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps the library-created scene live throughout the call.
        unsafe { Arc::increment_strong_count(scene) };
        TS_OK
    })
}
/// # Safety
/// Transfer exactly one live acquisition; NULL is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_release(scene: *const TsScene) {
    if !scene.is_null() {
        // SAFETY: caller transfers a library-created strong acquisition.
        drop(unsafe { Arc::from_raw(scene) });
    }
}
/// # Safety
/// Follow tessstep.h live handle and writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_get_info(scene: *const TsScene, out: *mut TsSceneInfo) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL output is valid writable C storage.
        if !unsafe { clear(out) } || scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: scene is live throughout this query.
        let s = unsafe { &*scene };
        // SAFETY: initialized writable output, scalar C record only.
        unsafe {
            out.write(TsSceneInfo {
                asset_count: s.scene.assets().len(),
                instance_count: s.scene.instances().len(),
            })
        };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_instance_at(
    scene: *const TsScene,
    index: usize,
    out: *mut TsSceneInstanceInfo,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL output is valid writable C storage.
        if !unsafe { clear(out) } || scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: scene is live throughout this query.
        let s = unsafe { &*scene };
        let Some(i) = s.scene.instances().get(index) else {
            return TS_NOT_FOUND;
        };
        let linear = i.world_transform.linear();
        let info = TsSceneInstanceInfo {
            source: input(i.source),
            world_linear: std::array::from_fn(|i| linear[i / 3][i % 3]),
            world_translation: i.world_transform.translation(),
            depth: i.depth as u64,
            mirrored: u32::from(i.mirrored),
            reserved: 0,
        };
        // SAFETY: initialized writable output, scalar C record only.
        unsafe { out.write(info) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and writable pointer slot contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_asset_mesh(
    scene: *const TsScene,
    id: u64,
    out: *mut *const TsMesh,
) -> u32 {
    // SAFETY: non-NULL output is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: scene is live throughout this query.
        let s = unsafe { &*scene };
        let Some(mesh) = s.assets.get(&AssetId(id)) else {
            return TS_NOT_FOUND;
        };
        let handle = Arc::into_raw(mesh.clone());
        // SAFETY: initialized writable slot; transfers one independent mesh acquisition.
        unsafe { out.write(handle) };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle, readable options and writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_scene_bake_instance(
    scene: *const TsScene,
    id: u64,
    options: *const TsMeshOptions,
    out: *mut *const TsMesh,
) -> u32 {
    // SAFETY: non-NULL output is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: non-NULL options points to a complete readable options record.
        let o = if options.is_null() {
            TsMeshOptions::default()
        } else {
            unsafe { *options }
        };
        let limits = match o.limits() {
            Ok(l) => l,
            Err(s) => return s,
        };
        // SAFETY: scene is live throughout baking.
        let s = unsafe { &*scene };
        let mesh = match s.scene.bake(InstanceId(id), limits) {
            Ok(m) => m,
            Err(e) => return status(e),
        };
        if o.require_solid != 0 && mesh.require_solid().is_err() {
            return TS_INVALID_MESH;
        }
        let handle = Arc::into_raw(Arc::new(TsMesh::from(mesh)));
        // SAFETY: initialized writable slot; publish only the successful result.
        unsafe { out.write(handle) };
        TS_OK
    })
}
