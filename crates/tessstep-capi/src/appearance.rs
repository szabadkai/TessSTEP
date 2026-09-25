use crate::{
    TS_INVALID_APPEARANCE, TS_INVALID_ARGUMENT, TS_NOT_FOUND, TS_OK, TS_RESOURCE_LIMIT, TsScene,
    boundary, clear, initialize,
};
use std::{ptr, sync::Arc};
use tessstep_mesh::{
    appearance::{self, Appearance, MaterialId, Target},
    scene::{AssetId, InstanceId},
};
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TsAppearanceOptions {
    pub struct_size: u32,
    pub abi_version: u32,
    pub max_materials: u64,
    pub max_bindings: u64,
    pub max_work: u64,
}
impl Default for TsAppearanceOptions {
    fn default() -> Self {
        let l = appearance::Limits::default();
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: 1,
            max_materials: l.max_materials as u64,
            max_bindings: l.max_bindings as u64,
            max_work: l.max_work as u64,
        }
    }
}
impl TsAppearanceOptions {
    fn limits(self) -> Result<appearance::Limits, u32> {
        if self.struct_size != size_of::<Self>() as u32 || self.abi_version != 1 {
            return Err(TS_INVALID_ARGUMENT);
        }
        let count = |v| usize::try_from(v).map_err(|_| TS_INVALID_ARGUMENT);
        Ok(appearance::Limits {
            max_materials: count(self.max_materials)?,
            max_bindings: count(self.max_bindings)?,
            max_work: count(self.max_work)?,
        })
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsMaterial {
    pub id: u64,
    pub rgba: [f64; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsStyleTarget {
    pub kind: u32,
    pub reserved: u32,
    pub id: u64,
    pub face_id: u64,
}
impl TsStyleTarget {
    fn target(self) -> Result<Target, u32> {
        if self.reserved != 0 {
            return Err(TS_INVALID_ARGUMENT);
        }
        Ok(match self.kind {
            1 if self.face_id == 0 => Target::Asset(AssetId(self.id)),
            2 => Target::AssetFace(AssetId(self.id), self.face_id),
            3 if self.face_id == 0 => Target::Instance(InstanceId(self.id)),
            4 => Target::InstanceFace(InstanceId(self.id), self.face_id),
            _ => return Err(TS_INVALID_ARGUMENT),
        })
    }
}
impl From<Target> for TsStyleTarget {
    fn from(target: Target) -> Self {
        let (kind, id, face_id) = match target {
            Target::Asset(id) => (1, id.0, 0),
            Target::AssetFace(id, face) => (2, id.0, face),
            Target::Instance(id) => (3, id.0, 0),
            Target::InstanceFace(id, face) => (4, id.0, face),
        };
        Self {
            kind,
            reserved: 0,
            id,
            face_id,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsStyleBinding {
    pub target: TsStyleTarget,
    pub material_id: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsResolvedMaterial {
    pub material: TsMaterial,
    pub source: TsStyleTarget,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TsAppearanceInfo {
    pub material_count: usize,
    pub binding_count: usize,
}
#[derive(Debug)]
pub struct TsAppearance {
    appearance: Appearance,
    scene: Arc<TsScene>,
}
/// # Safety
/// Follow tessstep.h writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_options_init(out: *mut TsAppearanceOptions) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is writable C options storage.
        if unsafe { clear(out) } {
            TS_OK
        } else {
            TS_INVALID_ARGUMENT
        }
    })
}
/// # Safety
/// Follow tessstep.h live scene, readable arrays/options and disjoint output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_create(
    scene: *const TsScene,
    materials: *const TsMaterial,
    material_count: usize,
    bindings: *const TsStyleBinding,
    binding_count: usize,
    options: *const TsAppearanceOptions,
    out: *mut *const TsAppearance,
) -> u32 {
    // SAFETY: non-NULL out is a valid writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if scene.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: non-NULL options is a complete readable record.
        let options = if options.is_null() {
            TsAppearanceOptions::default()
        } else {
            unsafe { *options }
        };
        let limits = match options.limits() {
            Ok(l) => l,
            Err(s) => return s,
        };
        let Some(input_work) = material_count.checked_add(binding_count) else {
            return TS_RESOURCE_LIMIT;
        };
        if material_count > limits.max_materials
            || binding_count > limits.max_bindings
            || input_work > limits.max_work
        {
            return TS_RESOURCE_LIMIT;
        }
        if (materials.is_null() && material_count != 0)
            || (bindings.is_null() && binding_count != 0)
            || material_count > isize::MAX as usize / size_of::<TsMaterial>()
            || binding_count > isize::MAX as usize / size_of::<TsStyleBinding>()
        {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: input arrays are aligned/readable for their documented lengths.
        let materials = if material_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(materials, material_count) }
        };
        // SAFETY: as above; NULL with zero count does not create a Rust slice.
        let bindings = if binding_count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(bindings, binding_count) }
        };
        let mut palette = Vec::with_capacity(material_count);
        for material in materials {
            let color = match appearance::LinearRgba::new(material.rgba) {
                Ok(c) => c,
                Err(_) => return TS_INVALID_APPEARANCE,
            };
            palette.push(appearance::Material {
                id: MaterialId(material.id),
                color,
            });
        }
        let mut assignments = Vec::with_capacity(binding_count);
        for binding in bindings {
            let target = match binding.target.target() {
                Ok(t) => t,
                Err(s) => return s,
            };
            assignments.push(appearance::Binding {
                target,
                material: MaterialId(binding.material_id),
            });
        }
        // SAFETY: scene is a live library handle. Acquire before constructing an Arc
        // so early returns release only this independent acquisition.
        let scene = unsafe {
            Arc::increment_strong_count(scene);
            Arc::from_raw(scene)
        };
        let limits = appearance::Limits {
            max_work: limits.max_work - input_work,
            ..limits
        };
        let appearance = match Appearance::new(scene.scene.clone(), palette, assignments, limits) {
            Ok(a) => a,
            Err(appearance::Error::ResourceLimit) => return TS_RESOURCE_LIMIT,
            Err(_) => return TS_INVALID_APPEARANCE,
        };
        let handle = Arc::into_raw(Arc::new(TsAppearance { appearance, scene }));
        // SAFETY: initialized output slot; publish ownership after complete validation.
        unsafe { out.write(handle) };
        TS_OK
    })
}
/// # Safety
/// Keep one live acquisition throughout the call; pair each retain with release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_retain(appearance: *const TsAppearance) -> u32 {
    boundary(|| {
        if appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: caller keeps the library-created handle live during this call.
        unsafe { Arc::increment_strong_count(appearance) };
        TS_OK
    })
}
/// # Safety
/// Transfer one live acquisition; NULL is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_release(appearance: *const TsAppearance) {
    if !appearance.is_null() {
        // SAFETY: caller transfers exactly one library-created acquisition.
        drop(unsafe { Arc::from_raw(appearance) });
    }
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_get_info(
    appearance: *const TsAppearance,
    out: *mut TsAppearanceInfo,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a writable C record.
        if !unsafe { clear(out) } || appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: appearance remains live throughout this query.
        let a = unsafe { &*appearance };
        // SAFETY: initialized output, C scalar record only.
        unsafe {
            out.write(TsAppearanceInfo {
                material_count: a.appearance.materials().len(),
                binding_count: a.appearance.bindings().len(),
            })
        };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_material_at(
    appearance: *const TsAppearance,
    index: usize,
    out: *mut TsMaterial,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a writable C record.
        if !unsafe { clear(out) } || appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: appearance remains live throughout this query.
        let a = unsafe { &*appearance };
        let Some(m) = a.appearance.materials().get(index) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: initialized output, C scalar record only.
        unsafe {
            out.write(TsMaterial {
                id: m.id.0,
                rgba: m.color.components(),
            })
        };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_binding_at(
    appearance: *const TsAppearance,
    index: usize,
    out: *mut TsStyleBinding,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a writable C record.
        if !unsafe { clear(out) } || appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: appearance remains live throughout this query.
        let a = unsafe { &*appearance };
        let Some(b) = a.appearance.bindings().get(index) else {
            return TS_NOT_FOUND;
        };
        // SAFETY: initialized output, C scalar record only.
        unsafe {
            out.write(TsStyleBinding {
                target: b.target.into(),
                material_id: b.material.0,
            })
        };
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable output contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_resolve_triangle(
    appearance: *const TsAppearance,
    instance_id: u64,
    triangle: usize,
    out: *mut TsResolvedMaterial,
) -> u32 {
    boundary(|| {
        // SAFETY: non-NULL out is a writable C record.
        if !unsafe { clear(out) } || appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: appearance remains live throughout this query.
        let a = unsafe { &*appearance };
        let resolved = match a
            .appearance
            .resolve_triangle(InstanceId(instance_id), triangle)
        {
            Ok(r) => r,
            Err(_) => return TS_NOT_FOUND,
        };
        if let Some(r) = resolved {
            // Validated assignments always refer to retained palette entries.
            let m = a
                .appearance
                .material(r.material)
                .expect("validated appearance material");
            // SAFETY: initialized output, C scalar records only; no kernel layouts exposed.
            unsafe {
                out.write(TsResolvedMaterial {
                    material: TsMaterial {
                        id: m.id.0,
                        rgba: m.color.components(),
                    },
                    source: r.source.into(),
                })
            };
        }
        TS_OK
    })
}
/// # Safety
/// Follow tessstep.h live handle and disjoint writable pointer slot contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_appearance_get_scene(
    appearance: *const TsAppearance,
    out: *mut *const TsScene,
) -> u32 {
    // SAFETY: non-NULL out is a writable pointer slot.
    if !unsafe { initialize(out, ptr::null()) } {
        return TS_INVALID_ARGUMENT;
    }
    boundary(|| {
        if appearance.is_null() {
            return TS_INVALID_ARGUMENT;
        }
        // SAFETY: appearance remains live throughout this query.
        let a = unsafe { &*appearance };
        let handle = Arc::into_raw(a.scene.clone());
        // SAFETY: initialized output; transfers an independent scene acquisition.
        unsafe { out.write(handle) };
        TS_OK
    })
}
