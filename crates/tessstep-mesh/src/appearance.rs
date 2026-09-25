//! Explicit linear RGBA appearance, independent of STEP styles and rendering.
//! Overrides share immutable scene geometry; unassigned triangles stay unstyled.
use crate::scene::{AssetId, InstanceId, Scene};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

/// Appearance-local nonzero identity, independent of asset and face IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaterialId(pub u64);
/// Linear-light RGB and straight (unassociated) opacity, all finite in `[0,1]`.
/// Alpha zero is fully transparent, alpha one opaque. No clamping or conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearRgba([f64; 4]);
impl LinearRgba {
    pub fn new(components: [f64; 4]) -> Result<Self, Error> {
        if components
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err(Error::InvalidColor);
        }
        Ok(Self(components))
    }
    pub fn components(self) -> [f64; 4] {
        self.0
    }
}
/// A color/opacity record, not a physically based lighting or texture model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub id: MaterialId,
    pub color: LinearRgba,
}
/// Exact identities only. Face zero is an ordinary face identity, not a wildcard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    Asset(AssetId),
    AssetFace(AssetId, u64),
    /// Overrides the entire occurrence and descendants until a nearer override.
    Instance(InstanceId),
    /// Applies only to this occurrence's own asset face; never inherited.
    InstanceFace(InstanceId, u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub target: Target,
    pub material: MaterialId,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_materials: usize,
    pub max_bindings: usize,
    /// Includes graph traversal and scans of distinct assets with face bindings.
    pub max_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_materials: 100_000,
            max_bindings: 1_000_000,
            max_work: 10_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidColor,
    InvalidMaterialId,
    DuplicateMaterial(MaterialId),
    MissingMaterial(MaterialId),
    DuplicateBinding(Target),
    MissingAsset(AssetId),
    MissingInstance(InstanceId),
    NoAsset(InstanceId),
    MissingFace(AssetId, u64),
    InvalidTriangle(InstanceId, usize),
    ResourceLimit,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "appearance: {self:?}")
    }
}
impl std::error::Error for Error {}
/// The winning material and original assignment, including ancestor provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub material: MaterialId,
    pub source: Target,
}
#[derive(Clone, Debug)]
pub struct Appearance {
    scene: Arc<Scene>,
    materials: Vec<Material>,
    bindings: Vec<Binding>,
    material_index: BTreeMap<MaterialId, usize>,
    assignments: BTreeMap<Target, MaterialId>,
    inherited: BTreeMap<InstanceId, Resolved>,
}
impl Appearance {
    /// All assignments are validated, even when overridden or on unused assets.
    /// No geometry buffers or per-occurrence triangle arrays are copied.
    pub fn new(
        scene: Arc<Scene>,
        materials: Vec<Material>,
        bindings: Vec<Binding>,
        limits: Limits,
    ) -> Result<Self, Error> {
        if materials.len() > limits.max_materials || bindings.len() > limits.max_bindings {
            return Err(Error::ResourceLimit);
        }
        let mut budget = crate::Budget(limits.max_work);
        let mut charge = |n| budget.charge(n).map_err(|_| Error::ResourceLimit);
        charge(materials.len())?;
        charge(bindings.len())?;
        let mut material_index = BTreeMap::new();
        for (i, material) in materials.iter().enumerate() {
            if material.id.0 == 0 {
                return Err(Error::InvalidMaterialId);
            }
            if material_index.insert(material.id, i).is_some() {
                return Err(Error::DuplicateMaterial(material.id));
            }
        }
        let mut assignments = BTreeMap::new();
        // Cache only the distinct face sets needed to validate face bindings.
        let mut face_sets = BTreeMap::<AssetId, BTreeSet<u64>>::new();
        for binding in &bindings {
            if !material_index.contains_key(&binding.material) {
                return Err(Error::MissingMaterial(binding.material));
            }
            if assignments
                .insert(binding.target, binding.material)
                .is_some()
            {
                return Err(Error::DuplicateBinding(binding.target));
            }
            let (asset, face) = match binding.target {
                Target::Asset(id) => (Some(id), None),
                Target::AssetFace(id, face) => (Some(id), Some(face)),
                Target::Instance(id) => {
                    scene.instance(id).ok_or(Error::MissingInstance(id))?;
                    (None, None)
                }
                Target::InstanceFace(id, face) => {
                    let instance = scene.instance(id).ok_or(Error::MissingInstance(id))?;
                    (
                        Some(instance.source.asset.ok_or(Error::NoAsset(id))?),
                        Some(face),
                    )
                }
            };
            if let Some(id) = asset {
                let asset = scene.asset(id).ok_or(Error::MissingAsset(id))?;
                if let Some(face) = face {
                    if let std::collections::btree_map::Entry::Vacant(entry) = face_sets.entry(id) {
                        let ids = &asset.mesh.data().face_ids;
                        charge(ids.len())?;
                        entry.insert(ids.iter().copied().collect());
                    }
                    if !face_sets[&id].contains(&face) {
                        return Err(Error::MissingFace(id, face));
                    }
                }
            }
        }
        // Scene already guarantees a forest. Walk once instead of revisiting ancestors
        // for every query; stack/owned data never recursively follow parent links.
        let count = scene.instances().len();
        charge(count)?;
        let index: BTreeMap<_, _> = scene
            .instances()
            .iter()
            .enumerate()
            .map(|(i, n)| (n.source.id, i))
            .collect();
        let mut children = vec![Vec::new(); count];
        let mut queue = VecDeque::new();
        for (i, node) in scene.instances().iter().enumerate() {
            charge(1)?;
            if let Some(parent) = node.source.parent {
                children[index[&parent]].push(i);
            } else {
                queue.push_back((i, None));
            }
        }
        let mut inherited = BTreeMap::new();
        while let Some((i, parent_style)) = queue.pop_front() {
            charge(1)?;
            let id = scene.instances()[i].source.id;
            let target = Target::Instance(id);
            let effective = assignments
                .get(&target)
                .map(|&material| Resolved {
                    material,
                    source: target,
                })
                .or(parent_style);
            if let Some(style) = effective {
                inherited.insert(id, style);
            }
            for &child in &children[i] {
                charge(1)?;
                queue.push_back((child, effective));
            }
        }
        Ok(Self {
            scene,
            materials,
            bindings,
            material_index,
            assignments,
            inherited,
        })
    }
    pub fn scene(&self) -> &Arc<Scene> {
        &self.scene
    }
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }
    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.material_index.get(&id).map(|&i| &self.materials[i])
    }
    /// Precedence: exact instance face, nearest instance/ancestor override,
    /// asset face, asset default. None means deliberately unspecified, not gray.
    /// Triangle order remains valid after Scene::bake (even under reflection).
    /// Allocation-free ordered lookups; no ancestor traversal or mesh scan.
    pub fn resolve_triangle(
        &self,
        id: InstanceId,
        triangle: usize,
    ) -> Result<Option<Resolved>, Error> {
        let instance = self.scene.instance(id).ok_or(Error::MissingInstance(id))?;
        let asset = instance.source.asset.ok_or(Error::NoAsset(id))?;
        let mesh = &self
            .scene
            .asset(asset)
            .ok_or(Error::MissingAsset(asset))?
            .mesh;
        let face = *mesh
            .data()
            .face_ids
            .get(triangle)
            .ok_or(Error::InvalidTriangle(id, triangle))?;
        let resolve = |target| {
            self.assignments.get(&target).map(|&material| Resolved {
                material,
                source: target,
            })
        };
        Ok(resolve(Target::InstanceFace(id, face))
            .or_else(|| self.inherited.get(&id).copied())
            .or_else(|| resolve(Target::AssetFace(asset, face)))
            .or_else(|| resolve(Target::Asset(asset))))
    }
}
