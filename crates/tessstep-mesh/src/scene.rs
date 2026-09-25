//! Reusable immutable mesh assets and explicitly placed occurrence forests.
//! Transforms are supplied by the caller; no STEP placement/defaulting is inferred.
use crate::{Mesh, MeshData};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tessstep_math::{Affine3, Direction3, ModelSpace, NumericalTolerance, Point3};

/// Scene-local identity. Zero is reserved for absence in the C protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetId(pub u64);
/// Identifies one occurrence, even when multiple occurrences use the same asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstanceId(pub u64);
/// Coordinates and translations use metres in each explicitly supplied local frame.
pub type Transform = Affine3<ModelSpace, ModelSpace>;
#[derive(Clone, Debug)]
pub struct Asset {
    pub id: AssetId,
    pub mesh: Arc<Mesh>,
}
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub id: InstanceId,
    pub parent: Option<InstanceId>,
    /// None represents an assembly/group node without its own geometry.
    pub asset: Option<AssetId>,
    /// Maps this occurrence's coordinates into its parent's coordinates.
    pub local_transform: Transform,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_assets: usize,
    pub max_instances: usize,
    pub max_depth: usize,
    pub max_work: usize,
    pub numerical: NumericalTolerance,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_assets: 100_000,
            max_instances: 1_000_000,
            max_depth: 1024,
            max_work: 10_000_000,
            numerical: NumericalTolerance::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    ResourceLimit,
    InvalidId,
    DuplicateAsset(AssetId),
    DuplicateInstance(InstanceId),
    MissingAsset(AssetId),
    MissingInstance(InstanceId),
    Cycle,
    NoAsset(InstanceId),
    Transform(InstanceId, tessstep_math::Error),
    Mesh(crate::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scene: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug)]
pub struct ResolvedInstance {
    pub source: Instance,
    pub world_transform: Transform,
    /// Renderers must reverse triangle winding for a mirrored occurrence.
    pub mirrored: bool,
    /// Roots have depth one.
    pub depth: usize,
}
#[derive(Clone, Debug)]
pub struct Scene {
    assets: Vec<Asset>,
    instances: Vec<ResolvedInstance>,
    asset_index: BTreeMap<AssetId, usize>,
    instance_index: BTreeMap<InstanceId, usize>,
    normal_matrices: Vec<[[f64; 3]; 3]>,
}
impl Scene {
    /// Resolve a forest iteratively, including disconnected components. Input order
    /// is retained; IDs never become allocation sizes. Mesh buffers are shared.
    pub fn new(
        assets: Vec<Asset>,
        instances: Vec<Instance>,
        limits: Limits,
    ) -> Result<Self, Error> {
        if assets.len() > limits.max_assets || instances.len() > limits.max_instances {
            return Err(Error::ResourceLimit);
        }
        let mut budget = crate::Budget(limits.max_work);
        let mut charge = |n| budget.charge(n).map_err(|_| Error::ResourceLimit);
        charge(assets.len())?;
        charge(instances.len())?;
        let mut asset_index = BTreeMap::new();
        for (i, asset) in assets.iter().enumerate() {
            if asset.id.0 == 0 {
                return Err(Error::InvalidId);
            }
            if asset_index.insert(asset.id, i).is_some() {
                return Err(Error::DuplicateAsset(asset.id));
            }
        }
        let mut instance_index = BTreeMap::new();
        for (i, instance) in instances.iter().enumerate() {
            if instance.id.0 == 0 {
                return Err(Error::InvalidId);
            }
            if instance_index.insert(instance.id, i).is_some() {
                return Err(Error::DuplicateInstance(instance.id));
            }
            if let Some(id) = instance.asset {
                if !asset_index.contains_key(&id) {
                    return Err(Error::MissingAsset(id));
                }
            }
        }
        let mut children = vec![Vec::new(); instances.len()];
        let mut queue = VecDeque::new();
        for (i, instance) in instances.iter().enumerate() {
            charge(1)?;
            if let Some(parent) = instance.parent {
                let &p = instance_index
                    .get(&parent)
                    .ok_or(Error::MissingInstance(parent))?;
                children[p].push(i);
            } else {
                queue.push_back((i, Transform::identity(), 1usize));
            }
        }
        let mut resolved = vec![None; instances.len()];
        let mut normal_matrices = vec![[[0.; 3]; 3]; instances.len()];
        let mut visited = 0;
        while let Some((i, parent_world, depth)) = queue.pop_front() {
            charge(1)?;
            if depth > limits.max_depth {
                return Err(Error::ResourceLimit);
            }
            let source = instances[i];
            let checked = || -> Result<_, tessstep_math::Error> {
                // Local and accumulated maps must both support inverse-transpose normals.
                prepare(source.local_transform, limits.numerical)?;
                let world = source.local_transform.then(parent_world)?;
                let (normal, mirrored) = prepare(world, limits.numerical)?;
                Ok((world, normal, mirrored))
            };
            let (world_transform, normal, mirrored) =
                checked().map_err(|e| Error::Transform(source.id, e))?;
            resolved[i] = Some(ResolvedInstance {
                source,
                world_transform,
                mirrored,
                depth,
            });
            normal_matrices[i] = normal;
            visited += 1;
            for &child in &children[i] {
                charge(1)?;
                queue.push_back((
                    child,
                    world_transform,
                    depth.checked_add(1).ok_or(Error::ResourceLimit)?,
                ));
            }
        }
        if visited != instances.len() {
            return Err(Error::Cycle);
        }
        Ok(Self {
            assets,
            instances: resolved.into_iter().flatten().collect(),
            asset_index,
            instance_index,
            normal_matrices,
        })
    }
    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }
    pub fn instances(&self) -> &[ResolvedInstance] {
        &self.instances
    }
    pub fn asset(&self, id: AssetId) -> Option<&Asset> {
        self.asset_index.get(&id).map(|&i| &self.assets[i])
    }
    pub fn instance(&self, id: InstanceId) -> Option<&ResolvedInstance> {
        self.instance_index.get(&id).map(|&i| &self.instances[i])
    }
    /// Explicitly copy one occurrence's mesh into world coordinates. Descendants
    /// are not merged. Face IDs remain asset-local; pair them with the instance ID.
    /// Reflections reverse triangles AND corner attributes; normals use inverse transpose.
    /// The resulting mesh is revalidated and may fail at extreme scales/coordinates.
    pub fn bake(&self, id: InstanceId, limits: crate::Limits) -> Result<Mesh, Error> {
        let &i = self
            .instance_index
            .get(&id)
            .ok_or(Error::MissingInstance(id))?;
        let instance = &self.instances[i];
        let asset = instance.source.asset.ok_or(Error::NoAsset(id))?;
        let data = self
            .asset(asset)
            .ok_or(Error::MissingAsset(asset))?
            .mesh
            .data();
        if data.positions.len() > limits.max_vertices || data.triangles.len() > limits.max_triangles
        {
            return Err(Error::ResourceLimit);
        }
        let mut budget = crate::Budget(limits.max_work);
        budget
            .charge(data.positions.len())
            .map_err(|_| Error::ResourceLimit)?;
        budget
            .charge(
                data.triangles
                    .len()
                    .checked_mul(4)
                    .ok_or(Error::ResourceLimit)?,
            )
            .map_err(|_| Error::ResourceLimit)?;
        let map_error = |e| Error::Transform(id, e);
        let positions = data
            .positions
            .iter()
            .map(|&p| {
                instance
                    .world_transform
                    .transform_point(Point3::new(p)?)
                    .map(|p| p.coordinates())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_error)?;
        let mut output = MeshData {
            positions,
            triangles: data.triangles.clone(),
            normals: Vec::with_capacity(data.normals.len()),
            uvs: data.uvs.clone(),
            face_ids: data.face_ids.clone(),
        };
        let matrix = self.normal_matrices[i];
        for (ti, corners) in data.normals.iter().enumerate() {
            let mut normals = [[0.; 3]; 3];
            for (k, n) in corners.iter().enumerate() {
                normals[k] = Direction3::<ModelSpace>::new(
                    matrix.map(|row| row.iter().zip(n).map(|(a, b)| a * b).sum()),
                )
                .map_err(map_error)?
                .vector()
                .components();
            }
            if instance.mirrored {
                output.triangles[ti].swap(1, 2);
                output.uvs[ti].swap(1, 2);
                normals.swap(1, 2);
            }
            output.normals.push(normals);
        }
        Mesh::new(
            output,
            crate::Limits {
                max_work: budget.0,
                ..limits
            },
        )
        .map_err(|e| {
            if e == crate::Error::ResourceLimit {
                Error::ResourceLimit
            } else {
                Error::Mesh(e)
            }
        })
    }
}
// Cache the inverse transpose once per occurrence. Determine handedness using
// scaled pivot elimination rather than an overflowing/underflowing determinant.
fn prepare(
    transform: Transform,
    tolerance: NumericalTolerance,
) -> Result<([[f64; 3]; 3], bool), tessstep_math::Error> {
    let inverse = transform.inverse(tolerance)?.linear();
    let normal = std::array::from_fn(|i| std::array::from_fn(|j| inverse[j][i]));
    let mut rows = transform.linear();
    let scale = rows.iter().flatten().fold(0_f64, |a, b| a.max(b.abs()));
    for row in &mut rows {
        for v in row {
            *v /= scale;
        }
    }
    let mut mirrored = false;
    for c in 0..3 {
        let mut pivot = c;
        for r in c + 1..3 {
            if rows[r][c].abs() > rows[pivot][c].abs() {
                pivot = r;
            }
        }
        if rows[pivot][c].abs() <= tolerance.relative() {
            return Err(tessstep_math::Error::SingularTransform);
        }
        if pivot != c {
            rows.swap(pivot, c);
            mirrored = !mirrored;
        }
        if rows[c][c] < 0. {
            mirrored = !mirrored;
        }
        let selected = rows[c];
        for row in rows.iter_mut().skip(c + 1) {
            let factor = row[c] / selected[c];
            for (value, pivot_value) in row.iter_mut().zip(selected).skip(c + 1) {
                *value -= factor * pivot_value;
            }
        }
    }
    Ok((normal, mirrored))
}
