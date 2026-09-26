//! Tessellated presentation graphics: a TESSELLATED_ANNOTATION_OCCURRENCE or
//! TESSELLATED_GEOMETRIC_SET as polylines, points and fill triangles, with
//! REPOSITIONED_TESSELLATED_ITEM placements applied. Graphics carry no mesh
//! validity, orientation or closure claim.
use super::{Context, Identity, decode_profile, winding_normal};
use crate::{Error, ErrorKind, ImportOptions};
use std::collections::BTreeMap;
use tessstep_math::{LengthUnit, ModelSpace, NumericalTolerance, Point, Vector};
use tessstep_model::{Document, decode::EntityView};
use tessstep_part21::{EntityId, ValueKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationItemKind {
    CurveSet,
    PointSet,
    SurfaceSet,
}
/// One leaf of the selected geometric set, once per use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationItem {
    pub entity: EntityId,
    pub kind: PresentationItemKind,
    /// The surface set supplies normals. They are checked for shape only and not used.
    pub supplied_normals: bool,
    /// REPOSITIONED_TESSELLATED_ITEM placements composed for this use, including its own.
    pub placements: usize,
}
/// Output and traversal budgets. Zero means zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationLimits {
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_polyline_points: usize,
    /// Geometric-set nesting, counting the selected set as one.
    pub max_depth: usize,
}
impl Default for PresentationLimits {
    fn default() -> Self {
        Self {
            max_vertices: 1_000_000,
            max_triangles: 2_000_000,
            max_polyline_points: 4_000_000,
            max_depth: 16,
        }
    }
}
/// Presentation graphics in metres, in the coordinate system of the representation
/// that contains the selected root. Polylines, points and triangles index
/// `positions`; each records the physical STEP ID of its source item.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedPresentation {
    root: EntityId,
    occurrence: Option<EntityId>,
    set: EntityId,
    styles: Vec<EntityId>,
    items: Vec<PresentationItem>,
    positions: Vec<[f64; 3]>,
    polyline_points: Vec<u32>,
    polyline_offsets: Vec<u64>,
    polyline_items: Vec<u64>,
    triangles: Vec<[u32; 3]>,
    triangle_items: Vec<u64>,
    points: Vec<u32>,
    point_items: Vec<u64>,
    skipped: usize,
    zero_area: usize,
    deviations: usize,
}
impl ImportedPresentation {
    pub fn root(&self) -> EntityId {
        self.root
    }
    /// The selected TESSELLATED_ANNOTATION_OCCURRENCE, if the root is one.
    pub fn occurrence(&self) -> Option<EntityId> {
        self.occurrence
    }
    /// The TESSELLATED_GEOMETRIC_SET that was traversed.
    pub fn geometric_set(&self) -> EntityId {
        self.set
    }
    /// The occurrence's style assignments, retained without decoding.
    pub fn styles(&self) -> &[EntityId] {
        &self.styles
    }
    pub fn items(&self) -> &[PresentationItem] {
        &self.items
    }
    pub fn positions(&self) -> &[[f64; 3]] {
        &self.positions
    }
    pub fn polyline_count(&self) -> usize {
        self.polyline_items.len()
    }
    /// Vertex indices of polyline `i`, in line-strip order.
    pub fn polyline(&self, i: usize) -> &[u32] {
        &self.polyline_points
            [self.polyline_offsets[i] as usize..self.polyline_offsets[i + 1] as usize]
    }
    /// Concatenated polyline vertex indices.
    pub fn polyline_points(&self) -> &[u32] {
        &self.polyline_points
    }
    /// `polyline_count() + 1` offsets into `polyline_points()`, starting at zero.
    pub fn polyline_offsets(&self) -> &[u64] {
        &self.polyline_offsets
    }
    /// Source TESSELLATED_CURVE_SET of each polyline.
    pub fn polyline_items(&self) -> &[u64] {
        &self.polyline_items
    }
    /// Fill triangles in the file's winding, which is not an orientation claim.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Source surface set of each triangle.
    pub fn triangle_items(&self) -> &[u64] {
        &self.triangle_items
    }
    pub fn points(&self) -> &[u32] {
        &self.points
    }
    /// Source TESSELLATED_POINT_SET of each point.
    pub fn point_items(&self) -> &[u64] {
        &self.point_items
    }
    /// Strip/fan triangles with a repeated index, omitted as in shape tessellations.
    pub fn skipped_degenerate(&self) -> usize {
        self.skipped
    }
    /// Retained triangles with distinct indices but collinear corners.
    pub fn zero_area_triangles(&self) -> usize {
        self.zero_area
    }
    /// Surface sets whose PNMAX understated NPOINTS, tolerated as in
    /// `import_tessellated`.
    pub fn pnmax_deviations(&self) -> usize {
        self.deviations
    }
}

/// Import a selected TESSELLATED_ANNOTATION_OCCURRENCE (whose styled item must be a
/// TESSELLATED_GEOMETRIC_SET) or a TESSELLATED_GEOMETRIC_SET. Children are
/// TESSELLATED_CURVE_SETs (one polyline per line strip), TESSELLATED_POINT_SETs,
/// triangulated surface sets and nested geometric sets, traversed depth first in
/// child order. REPOSITIONED_TESSELLATED_ITEM locations are composed from the root
/// down and applied to positions. Vertex identity is (coordinate list, point index,
/// placement), never proximity. Indices, counts and strips are checked as for shape
/// tessellations (including the tolerated PNMAX deviation), and a line strip may not
/// repeat a consecutive point. Supplied
/// normals are checked for count and shape but not used, and fill triangles are not
/// required to be manifold, oriented or of positive area: exporters write annotation
/// normals in inconsistent frames and fills with collinear corners (see
/// EXISTING_TESSELLATIONS.md). Styles are retained as entity IDs without decoding.
pub fn import_presentation(
    document: &Document,
    root: EntityId,
    unit: LengthUnit,
    options: ImportOptions,
    limits: PresentationLimits,
) -> Result<ImportedPresentation, Error> {
    let decoded = decode_profile(document, root, options)?;
    let mut c = Context::new(
        &decoded,
        root,
        options,
        unit,
        tessstep_mesh::Limits::default(),
    );
    let view = c.view(root)?;
    let (occurrence, set, styles) = if c.is(view, "TESSELLATED_ANNOTATION_OCCURRENCE") {
        let mut styles = Vec::new();
        for style in c.aggregate(view, "STYLES")? {
            c.charge(1)?;
            if let ValueKind::Reference(id) = style.kind {
                styles.push(id);
            }
        }
        let item = c.target(c.attr(view, "ITEM")?)?;
        if !c.is(item, "TESSELLATED_GEOMETRIC_SET") {
            return Err(c.error(
                ErrorKind::Unsupported,
                "annotation occurrence does not style a tessellated geometric set",
            ));
        }
        (Some(root), item, styles)
    } else if c.is(view, "TESSELLATED_GEOMETRIC_SET") {
        (None, view, Vec::new())
    } else {
        return Err(c.error(
            ErrorKind::Unsupported,
            "selected root is not a tessellated annotation occurrence or geometric set",
        ));
    };
    let mut g = Graphics {
        limits,
        placements: vec![Placement::IDENTITY],
        vertices: BTreeMap::new(),
        out: ImportedPresentation {
            root,
            occurrence,
            set: set.id,
            styles,
            items: Vec::new(),
            positions: Vec::new(),
            polyline_points: Vec::new(),
            polyline_offsets: vec![0],
            polyline_items: Vec::new(),
            triangles: Vec::new(),
            triangle_items: Vec::new(),
            points: Vec::new(),
            point_items: Vec::new(),
            skipped: 0,
            zero_area: 0,
            deviations: 0,
        },
    };
    // (item, parent placement, depth, composed placement count). Children are pushed
    // in reverse so they pop in document child order; a shared child is emitted once
    // per use.
    let mut stack = vec![(set, 0usize, 1usize, 0usize)];
    while let Some((node, parent, depth, count)) = stack.pop() {
        c.current = node.id;
        c.charge(1)?;
        if depth > limits.max_depth {
            return Err(c.error(ErrorKind::ResourceLimit, "presentation nesting depth"));
        }
        let (placement, count) = if c.is(node, "REPOSITIONED_TESSELLATED_ITEM") {
            let location = c.target(c.attr(node, "LOCATION")?)?;
            let local = frame(&mut c, location)?;
            c.current = node.id;
            let composed = g.placements[parent]
                .then(&local)
                .ok_or_else(|| c.invalid("placement composition is not finite"))?;
            g.placements.push(composed);
            (g.placements.len() - 1, count + 1)
        } else {
            (parent, count)
        };
        if c.is(node, "TESSELLATED_GEOMETRIC_SET") {
            let children = c.aggregate(node, "CHILDREN")?;
            for child in children.iter().rev() {
                c.charge(1)?;
                stack.push((c.target(child)?, placement, depth + 1, count));
            }
            continue;
        }
        let (kind, supplied_normals) = if c.is(node, "TESSELLATED_CURVE_SET") {
            g.curves(&mut c, node, placement)?;
            (PresentationItemKind::CurveSet, false)
        } else if c.is(node, "TESSELLATED_POINT_SET") {
            g.point_set(&mut c, node, placement)?;
            (PresentationItemKind::PointSet, false)
        } else if c.is(node, "TESSELLATED_SURFACE_SET") {
            let supplied = g.surface(&mut c, node, placement)?;
            (PresentationItemKind::SurfaceSet, supplied)
        } else {
            return Err(c.error(
                ErrorKind::Unsupported,
                "geometric set child is not a curve, point, surface or geometric set",
            ));
        };
        g.out.items.push(PresentationItem {
            entity: node.id,
            kind,
            supplied_normals,
            placements: count,
        });
    }
    g.out.skipped = c.skipped;
    g.out.deviations = c.deviations;
    Ok(g.out)
}

/// Row-major linear map and translation in metres: p' = linear * p + translation.
#[derive(Clone, Copy, Debug)]
struct Placement {
    linear: [[f64; 3]; 3],
    translation: [f64; 3],
}
impl Placement {
    const IDENTITY: Self = Self {
        linear: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        translation: [0.; 3],
    };
    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let mut out = self.translation;
        for (i, o) in out.iter_mut().enumerate() {
            *o += (0..3).map(|j| self.linear[i][j] * p[j]).sum::<f64>();
        }
        out
    }
    /// `self` after `local`: maps local coordinates into self's parent system.
    fn then(&self, local: &Self) -> Option<Self> {
        let mut linear = [[0.; 3]; 3];
        for (i, row) in linear.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..3).map(|k| self.linear[i][k] * local.linear[k][j]).sum();
            }
        }
        let composed = Self {
            linear,
            translation: self.apply(local.translation),
        };
        let finite = composed.linear.iter().flatten().all(|x| x.is_finite())
            && composed.translation.iter().all(|x| x.is_finite());
        finite.then_some(composed)
    }
}

/// An AXIS2_PLACEMENT_3D as a rigid placement, with ISO 10303-42 defaults and the
/// reference direction projected orthogonal to the axis.
fn frame(c: &mut Context<'_, '_>, placement: &EntityView<'_>) -> Result<Placement, Error> {
    c.current = placement.id;
    c.charge(1)?;
    let direction =
        |c: &Context<'_, '_>, v: &EntityView<'_>| -> Result<Vector<ModelSpace, 3>, Error> {
            let v = c.checked(Vector::new(c.numbers(c.attr(v, "DIRECTION_RATIOS")?)?))?;
            Ok(c.checked(v.normalized())?.vector())
        };
    let location = c.target(c.attr(placement, "LOCATION")?)?;
    let mut origin = c.numbers(c.attr(location, "COORDINATES")?)?;
    for x in &mut origin {
        *x = c.checked(c.unit.to_metres(*x))?;
    }
    let axis = c.attr(placement, "AXIS")?;
    let z = if matches!(axis.kind, ValueKind::Null) {
        c.checked(Vector::new([0., 0., 1.]))?
    } else {
        direction(c, c.target(axis)?)?
    };
    let reference = c.attr(placement, "REF_DIRECTION")?;
    let default = matches!(reference.kind, ValueKind::Null);
    let mut x = if default {
        c.checked(Vector::new([1., 0., 0.]))?
    } else {
        direction(c, c.target(reference)?)?
    };
    c.current = placement.id;
    if c.checked(c.checked(z.cross(x))?.norm())? <= NumericalTolerance::default().relative() {
        if !default {
            return Err(c.invalid("parallel placement axes"));
        }
        x = c.checked(Vector::new([0., 1., 0.]))?;
    }
    let projection = c.checked(z.scaled(c.checked(z.dot(x))?))?;
    x = c
        .checked(c.checked(x.subtracted(projection))?.normalized())?
        .vector();
    let y = c.checked(z.cross(x))?;
    let [x, y, z] = [x, y, z].map(|v| v.components());
    Ok(Placement {
        linear: [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]],
        translation: origin,
    })
}

struct Graphics {
    limits: PresentationLimits,
    placements: Vec<Placement>,
    vertices: BTreeMap<(Identity, usize), u32>,
    out: ImportedPresentation,
}
impl Graphics {
    fn vertex(
        &mut self,
        c: &mut Context<'_, '_>,
        identity: Identity,
        placement: usize,
    ) -> Result<u32, Error> {
        if let Some(&id) = self.vertices.get(&(identity, placement)) {
            return Ok(id);
        }
        let id = self.out.positions.len();
        if id >= self.limits.max_vertices {
            return Err(c.error(ErrorKind::ResourceLimit, "presentation vertex budget"));
        }
        let id = u32::try_from(id)
            .map_err(|_| c.error(ErrorKind::ResourceLimit, "presentation vertex budget"))?;
        let xyz = self.placements[placement].apply(c.position(identity)?);
        c.checked(Point::<ModelSpace, 3>::new(xyz))?;
        self.out.positions.push(xyz);
        self.vertices.insert((identity, placement), id);
        Ok(id)
    }
    fn curves<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        node: &'d EntityView<'a>,
        placement: usize,
    ) -> Result<(), Error> {
        c.record()?;
        let list = c.target(c.attr(node, "COORDINATES")?)?;
        let points = c.coordinates(list)?;
        for strip in c.aggregate(node, "LINE_STRIPS")? {
            let strip = c.indices(
                strip,
                points.len(),
                "point index outside the coordinate list",
            )?;
            if strip.windows(2).any(|w| w[0] == w[1]) {
                return Err(c.invalid("line strip repeats a consecutive point"));
            }
            for k in strip {
                if self.out.polyline_points.len() >= self.limits.max_polyline_points {
                    return Err(c.error(ErrorKind::ResourceLimit, "presentation polyline budget"));
                }
                let v = self.vertex(c, (list.id, k), placement)?;
                self.out.polyline_points.push(v);
            }
            self.out
                .polyline_offsets
                .push(self.out.polyline_points.len() as u64);
            self.out.polyline_items.push(node.id.get());
        }
        Ok(())
    }
    fn point_set<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        node: &'d EntityView<'a>,
        placement: usize,
    ) -> Result<(), Error> {
        c.record()?;
        let list = c.target(c.attr(node, "COORDINATES")?)?;
        let points = c.coordinates(list)?;
        for k in c.indices(
            c.attr(node, "POINT_LIST")?,
            points.len(),
            "point index outside the coordinate list",
        )? {
            if self.out.points.len() >= self.limits.max_vertices {
                return Err(c.error(ErrorKind::ResourceLimit, "presentation vertex budget"));
            }
            let v = self.vertex(c, (list.id, k), placement)?;
            self.out.points.push(v);
            self.out.point_items.push(node.id.get());
        }
        Ok(())
    }
    fn surface<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        node: &'d EntityView<'a>,
        placement: usize,
    ) -> Result<bool, Error> {
        let plan = c.plan(node, false)?;
        c.current = node.id;
        for t in &plan.triangles {
            c.charge(1)?;
            if self.out.triangles.len() >= self.limits.max_triangles {
                return Err(c.error(ErrorKind::ResourceLimit, "presentation triangle budget"));
            }
            let mut corners = [0; 3];
            for (corner, &k) in corners.iter_mut().zip(t) {
                *corner = self.vertex(c, plan.identity(k), placement)?;
            }
            if winding_normal(corners.map(|i| self.out.positions[i as usize])).is_err() {
                self.out.zero_area += 1;
            }
            self.out.triangles.push(corners);
            self.out.triangle_items.push(node.id.get());
        }
        Ok(plan.supplied)
    }
}
