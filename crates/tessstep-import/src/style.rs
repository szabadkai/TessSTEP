//! Presentation styles: surface colour and transparency of styled shapes, adapted
//! to the Milestone 19 appearance model over a linked assembly's scene.
//!
//! Styled items are decoded with the bundled original style profile
//! (`corpus/geometry/style.exp`); their targets are retained as links and matched to
//! imported shapes by STEP identity: a solid root or its outer shell styles the asset,
//! a face styles that face of its asset (mesh face IDs are STEP face IDs), and a
//! shape representation styles every imported root it holds. Surface colour comes
//! from a fill-area colour, else from the surface rendering colour; opacity is one
//! minus the rendering transparency. RGB values are read as sRGB-encoded and
//! converted to linear light. Curve, point and occurrence-specific styles, layers
//! and invisibility are counted as unsupported, never applied.
use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;
use tessstep_mesh::appearance::{Appearance, Binding, LinearRgba, Material, MaterialId, Target};
use tessstep_mesh::scene::AssetId;

#[derive(Clone, Copy, Debug, Default)]
pub struct StyleOptions {
    pub import: ImportOptions,
    pub appearance: tessstep_mesh::appearance::Limits,
}

/// How the document's styled items were used.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyleCounts {
    /// STYLED_ITEM and OVER_RIDING_STYLED_ITEM records.
    pub styled_items: usize,
    /// Styled items applied to an asset (solid, outer shell or representation).
    pub asset_styles: usize,
    /// Styled items applied to a face of an asset.
    pub face_styles: usize,
    /// Styled items with a surface colour whose target is a solid, shell, face or
    /// shape representation that was not imported or not placed (for example a face
    /// of a root that failed to import, or of a shell-based surface model).
    pub unimported: usize,
    /// Styled items with a surface colour on other geometry (curves, edges, points,
    /// surfaces and surface models), by the target's STEP record name.
    pub unsupported_targets: BTreeMap<String, usize>,
    /// Styled items whose assignments hold no surface colour (curve and point
    /// styles only); their targets are curves, points or annotation geometry.
    pub no_surface_colour: usize,
    /// Styles valid only in a context or for one occurrence: presentation styles by
    /// context and CONTEXT_DEPENDENT_OVER_RIDING_STYLED_ITEM records.
    pub context_styles: usize,
    /// Targets whose styles disagree; the over-riding, then the more direct, then
    /// the first style in document order was kept.
    pub conflicts: usize,
    /// Styled item records that are part of a complex instance (annotation
    /// occurrences and similar), which are not shape styles.
    pub complex_items: usize,
    /// PRESENTATION_LAYER_ASSIGNMENT and INVISIBILITY records, not interpreted.
    pub layers: usize,
    pub invisibility: usize,
}

/// Imported surface colours of a linked assembly.
#[derive(Clone, Debug)]
pub struct ImportedAppearance {
    appearance: Arc<Appearance>,
    counts: StyleCounts,
    excluded: Vec<ExcludedRecord>,
}
impl ImportedAppearance {
    /// Materials are the distinct colours in document order of first use.
    pub fn appearance(&self) -> &Arc<Appearance> {
        &self.appearance
    }
    pub fn counts(&self) -> &StyleCounts {
        &self.counts
    }
    /// Styled items outside the style profile, with their typed reason.
    pub fn excluded(&self) -> &[ExcludedRecord] {
        &self.excluded
    }
}

/// Adapt the document's surface styles to an appearance over `assembly`'s scene.
pub fn import_appearance(
    document: &Document,
    assembly: &LinkedAssembly,
    options: StyleOptions,
) -> Result<ImportedAppearance, Error> {
    let mut work = options.import.max_work;
    let mut charge = |n: usize| -> Result<(), Error> {
        work = work.checked_sub(n).ok_or_else(|| {
            style_error(
                ErrorKind::ResourceLimit,
                None,
                &format!(
                    "style work budget exceeded (max_work {})",
                    options.import.max_work
                ),
            )
        })?;
        Ok(())
    };
    let scene = assembly.scene();
    // Shape identities: roots, their outer shells and their faces.
    let mut assets = BTreeSet::new();
    let mut faces: BTreeMap<u64, AssetId> = BTreeMap::new();
    for asset in scene.assets() {
        assets.insert(asset.id);
        for &face in &asset.mesh.data().face_ids {
            charge(1)?;
            faces.entry(face).or_insert(asset.id);
        }
    }
    let mut shells: BTreeMap<EntityId, AssetId> = BTreeMap::new();
    let mut counts = StyleCounts::default();
    let mut roots = Vec::new();
    let mut styled = BTreeSet::new();
    for e in document.entities().iter() {
        charge(1)?;
        let records = e.kind.records();
        let name = |i: usize| records[i].name.as_ref();
        match records {
            [r] => {
                let n = r.name.as_ref();
                if n.eq_ignore_ascii_case("STYLED_ITEM")
                    || n.eq_ignore_ascii_case("OVER_RIDING_STYLED_ITEM")
                {
                    roots.push(e.id);
                    styled.insert(e.id);
                } else if [
                    "SURFACE_STYLE_USAGE",
                    "SURFACE_STYLE_FILL_AREA",
                    "SURFACE_STYLE_RENDERING",
                    "SURFACE_STYLE_RENDERING_WITH_PROPERTIES",
                ]
                .iter()
                .any(|k| n.eq_ignore_ascii_case(k))
                {
                    // Linked from assignments and side styles; decoded as roots so
                    // curve and point styles beside them are never decoded.
                    roots.push(e.id);
                } else if n.eq_ignore_ascii_case("CONTEXT_DEPENDENT_OVER_RIDING_STYLED_ITEM") {
                    counts.styled_items += 1;
                    counts.context_styles += 1;
                } else if n.eq_ignore_ascii_case("PRESENTATION_LAYER_ASSIGNMENT") {
                    counts.layers += 1;
                } else if n.eq_ignore_ascii_case("INVISIBILITY") {
                    counts.invisibility += 1;
                } else if assets.contains(&AssetId(e.id.get()))
                    && ["MANIFOLD_SOLID_BREP", "BREP_WITH_VOIDS", "FACETED_BREP"]
                        .iter()
                        .any(|k| n.eq_ignore_ascii_case(k))
                {
                    // The outer shell is the solid's second parameter.
                    if let Some(StepValue {
                        kind: ValueKind::Reference(shell),
                        ..
                    }) = r.parameters.get(1)
                    {
                        shells.insert(*shell, AssetId(e.id.get()));
                    }
                }
            }
            _ if (0..records.len()).any(|i| name(i).eq_ignore_ascii_case("STYLED_ITEM")) => {
                counts.complex_items += 1;
            }
            _ => {}
        }
    }
    let (decoded, mut excluded) = decode_styles(document, &roots, options.import)?;
    counts.styled_items += excluded
        .iter()
        .filter(|e| styled.contains(&e.record))
        .count();
    let mut chosen: BTreeMap<Target, (u8, [u64; 4], usize)> = BTreeMap::new();
    if let Some(decoded) = &decoded {
        let mut cx = Styles {
            decoded,
            charge: &mut charge,
        };
        for (order, view) in decoded.entities().iter().enumerate() {
            if !styled.contains(&view.id) {
                continue;
            }
            counts.styled_items += 1;
            let over_riding = cx.is(view, "OVER_RIDING_STYLED_ITEM")?;
            // An invalid colour or transparency excludes only its own styled item.
            let (colour, by_context) = match cx.surface_colour(view) {
                Ok(found) => found,
                Err(e) if e.kind == ErrorKind::ResourceLimit => return Err(e),
                Err(e) => {
                    excluded.push(ExcludedRecord {
                        record: view.id,
                        structural: false,
                        error: Error {
                            message: format!("styled item #{}: {}", view.id.get(), e.message),
                            ..e
                        },
                    });
                    continue;
                }
            };
            let Some(colour) = colour else {
                if by_context {
                    counts.context_styles += 1;
                } else {
                    counts.no_surface_colour += 1;
                }
                continue;
            };
            let ValueKind::Reference(item) = cx.attr(view, "ITEM").kind else {
                unreachable!("decoded required reference");
            };
            let asset = AssetId(item.get());
            let (targets, direct): (Vec<Target>, bool) = if assets.contains(&asset) {
                (vec![Target::Asset(asset)], true)
            } else if let Some(&owner) = faces.get(&item.get()) {
                (vec![Target::AssetFace(owner, item.get())], true)
            } else if let Some(&owner) = shells.get(&item) {
                (vec![Target::Asset(owner)], false)
            } else if let Some(held) = assembly.held.get(&item) {
                let found: Vec<Target> = held
                    .iter()
                    .map(|r| AssetId(r.get()))
                    .filter(|a| assets.contains(a))
                    .map(Target::Asset)
                    .collect();
                (found, false)
            } else {
                (Vec::new(), false)
            };
            if targets.is_empty() {
                let records = document.entities().get(item).map(|e| e.kind.records());
                let name = match records {
                    Some([r]) => r.name.as_ref().to_ascii_uppercase(),
                    Some(_) => "complex instance".into(),
                    None => "missing".into(),
                };
                if SHAPES.iter().any(|s| name == *s) || name.ends_with("SHAPE_REPRESENTATION") {
                    counts.unimported += 1;
                } else {
                    *counts.unsupported_targets.entry(name).or_default() += 1;
                }
                continue;
            }
            if matches!(targets[0], Target::AssetFace(..)) {
                counts.face_styles += 1;
            } else {
                counts.asset_styles += 1;
            }
            let rank = u8::from(over_riding) * 2 + u8::from(direct);
            let bits = colour.map(f64::to_bits);
            for target in targets {
                cx.tick()?;
                match chosen.get(&target) {
                    Some(&(r, b, _)) if r > rank || (r == rank && b == bits) => {}
                    Some(&(r, _, _)) if r == rank => counts.conflicts += 1,
                    Some(_) | None => {
                        chosen.insert(target, (rank, bits, order));
                    }
                }
            }
        }
    }
    // Materials in document order of first use; bindings in target order.
    let mut by_order: Vec<(usize, Target, [u64; 4])> =
        chosen.into_iter().map(|(t, (_, b, o))| (o, t, b)).collect();
    by_order.sort();
    let mut palette: BTreeMap<[u64; 4], MaterialId> = BTreeMap::new();
    let mut materials = Vec::new();
    let mut bindings = Vec::new();
    for (_, target, bits) in by_order {
        charge(1)?;
        let next = MaterialId(materials.len() as u64 + 1);
        let id = *palette.entry(bits).or_insert_with(|| {
            materials.push((next, bits));
            next
        });
        bindings.push(Binding {
            target,
            material: id,
        });
    }
    bindings.sort_by_key(|b| b.target);
    let materials = materials
        .into_iter()
        .map(|(id, bits)| {
            LinearRgba::new(bits.map(f64::from_bits))
                .map(|color| Material { id, color })
                .map_err(|e| style_error(ErrorKind::InvalidGeometry, None, &e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let appearance = Appearance::new(scene.clone(), materials, bindings, options.appearance)
        .map_err(|e| {
            style_error(
                if e == tessstep_mesh::appearance::Error::ResourceLimit {
                    ErrorKind::ResourceLimit
                } else {
                    ErrorKind::InvalidGeometry
                },
                None,
                &e.to_string(),
            )
        })?;
    Ok(ImportedAppearance {
        appearance: Arc::new(appearance),
        counts,
        excluded,
    })
}

/// Shape records whose styles apply when the shape is imported.
const SHAPES: [&str; 12] = [
    "MANIFOLD_SOLID_BREP",
    "BREP_WITH_VOIDS",
    "FACETED_BREP",
    "CLOSED_SHELL",
    "OPEN_SHELL",
    "ADVANCED_FACE",
    "FACE_SURFACE",
    "TESSELLATED_SOLID",
    "TESSELLATED_SHELL",
    "TRIANGULATED_FACE",
    "COMPLEX_TRIANGULATED_FACE",
    "SHELL_BASED_SURFACE_MODEL",
];

struct Styles<'d, 'a, F> {
    decoded: &'d DecodedDocument<'a>,
    charge: &'d mut F,
}
impl<'d, 'a, F: FnMut(usize) -> Result<(), Error>> Styles<'d, 'a, F> {
    fn tick(&mut self) -> Result<(), Error> {
        (self.charge)(1)
    }
    fn is(&mut self, view: &EntityView<'_>, name: &str) -> Result<bool, Error> {
        self.tick()?;
        Ok(view.types.iter().any(|id| {
            self.decoded
                .schemas()
                .declaration(*id)
                .is_some_and(|d| d.name.eq_ignore_ascii_case(name))
        }))
    }
    fn attr(&self, view: &'d EntityView<'a>, name: &str) -> &'a StepValue {
        view.attributes
            .iter()
            .find(|a| a.declaration.name.eq_ignore_ascii_case(name))
            .map(|a| a.value)
            .expect("profile attribute")
    }
    fn view(&self, value: &StepValue) -> Option<&'d EntityView<'a>> {
        match &value.kind {
            ValueKind::Reference(id) => self.decoded.get(*id),
            ValueKind::Typed { value, .. } => self.view(value),
            _ => None,
        }
    }
    fn items(&self, view: &'d EntityView<'a>, name: &str) -> Vec<&'d EntityView<'a>> {
        match &self.attr(view, name).kind {
            ValueKind::Aggregate(values) => values.iter().filter_map(|v| self.view(v)).collect(),
            _ => Vec::new(),
        }
    }
    fn number(value: &StepValue) -> Option<f64> {
        match &value.kind {
            ValueKind::Real(x) => Some(*x),
            ValueKind::Integer(x) => Some(*x as f64),
            ValueKind::Typed { value, .. } => Self::number(value),
            _ => None,
        }
    }
    /// A colour as linear RGB, or None when it is not an RGB or known pre-defined colour.
    fn colour(&mut self, value: &StepValue) -> Result<Option<[f64; 3]>, Error> {
        let Some(view) = self.view(value) else {
            return Ok(None);
        };
        let rgb = if self.is(view, "COLOUR_RGB")? {
            let c = ["RED", "GREEN", "BLUE"].map(|n| Self::number(self.attr(view, n)));
            match c {
                [Some(r), Some(g), Some(b)] => [r, g, b],
                _ => return Ok(None),
            }
        } else if self.is(view, "DRAUGHTING_PRE_DEFINED_COLOUR")? {
            let ValueKind::String(name) = &self.attr(view, "NAME").kind else {
                return Ok(None);
            };
            match pre_defined(name) {
                Some(rgb) => rgb,
                None => return Ok(None),
            }
        } else {
            return Ok(None);
        };
        if rgb
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err(style_error(
                ErrorKind::InvalidGeometry,
                Some(view.id),
                "colour component outside [0,1]",
            ));
        }
        Ok(Some(rgb.map(srgb_to_linear)))
    }
    /// The surface colour and opacity of a styled item's assignments, and whether a
    /// context-dependent assignment was skipped.
    fn surface_colour(
        &mut self,
        view: &'d EntityView<'a>,
    ) -> Result<(Option<[f64; 4]>, bool), Error> {
        let mut fill = None;
        let mut rendered = None;
        let mut transparency = None;
        let mut by_context = false;
        for assignment in self.items(view, "STYLES") {
            self.tick()?;
            if self.is(assignment, "PRESENTATION_STYLE_BY_CONTEXT")? {
                by_context = true;
                continue;
            }
            for usage in self.items(assignment, "STYLES") {
                self.tick()?;
                if !self.is(usage, "SURFACE_STYLE_USAGE")? {
                    continue;
                }
                let Some(side) = self.view(self.attr(usage, "STYLE")) else {
                    continue;
                };
                for element in self.items(side, "STYLES") {
                    self.tick()?;
                    if self.is(element, "SURFACE_STYLE_FILL_AREA")? {
                        let Some(area) = self.view(self.attr(element, "FILL_AREA")) else {
                            continue;
                        };
                        for fill_style in self.items(area, "FILL_STYLES") {
                            if fill.is_none() {
                                fill = self.colour(self.attr(fill_style, "FILL_COLOUR"))?;
                            }
                        }
                    } else if self.is(element, "SURFACE_STYLE_RENDERING")? {
                        if rendered.is_none() {
                            rendered = self.colour(self.attr(element, "SURFACE_COLOUR"))?;
                        }
                        if self.is(element, "SURFACE_STYLE_RENDERING_WITH_PROPERTIES")? {
                            for property in self.items(element, "PROPERTIES") {
                                if self.is(property, "SURFACE_STYLE_TRANSPARENT")? {
                                    let t = Self::number(self.attr(property, "TRANSPARENCY"));
                                    match t {
                                        Some(t) if (0. ..=1.).contains(&t) => {
                                            transparency.get_or_insert(t);
                                        }
                                        _ => {
                                            return Err(style_error(
                                                ErrorKind::InvalidGeometry,
                                                Some(property.id),
                                                "transparency outside [0,1]",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let alpha = 1. - transparency.unwrap_or(0.);
        Ok((
            fill.or(rendered).map(|[r, g, b]| [r, g, b, alpha]),
            by_context,
        ))
    }
}

/// The ISO 10303-46 pre-defined colours.
fn pre_defined(name: &str) -> Option<[f64; 3]> {
    [
        ("black", [0., 0., 0.]),
        ("red", [1., 0., 0.]),
        ("green", [0., 1., 0.]),
        ("blue", [0., 0., 1.]),
        ("yellow", [1., 1., 0.]),
        ("magenta", [1., 0., 1.]),
        ("cyan", [0., 1., 1.]),
        ("white", [1., 1., 1.]),
    ]
    .into_iter()
    .find(|(n, _)| name.eq_ignore_ascii_case(n))
    .map(|(_, rgb)| rgb)
}

/// The sRGB transfer function, inverted: encoded display value to linear light.
fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn decode_styles<'a>(
    document: &'a Document,
    roots: &[EntityId],
    options: ImportOptions,
) -> Result<(Option<DecodedDocument<'a>>, Vec<ExcludedRecord>), Error> {
    use style_profile::schema_tessstep_style as s;
    use tessstep_schema::EntityBinding;
    if roots.is_empty() {
        return Ok((None, Vec::new()));
    }
    let decode = |roots: &[EntityId]| {
        decode::decode_reachable_profile_with_links(
            document,
            &style_profile::SCHEMA_SET,
            "tessstep_style",
            roots,
            &[],
            &[
                decode::LinkSlot {
                    entity: s::Entity_STYLED_ITEM::DECLARATION,
                    attribute: "ITEM",
                },
                decode::LinkSlot {
                    entity: s::Entity_PRESENTATION_STYLE_BY_CONTEXT::DECLARATION,
                    attribute: "STYLE_CONTEXT",
                },
                decode::LinkSlot {
                    entity: s::Entity_PRESENTATION_STYLE_ASSIGNMENT::DECLARATION,
                    attribute: "STYLES",
                },
                decode::LinkSlot {
                    entity: s::Entity_SURFACE_SIDE_STYLE::DECLARATION,
                    attribute: "STYLES",
                },
            ],
            decode::Limits {
                max_work: options.max_work,
                ..decode::Limits::default()
            },
        )
    };
    let error = |e: decode::Error| Error {
        kind: match e.kind {
            decode::ErrorKind::ResourceLimit => ErrorKind::ResourceLimit,
            decode::ErrorKind::MissingReference => ErrorKind::MissingEntity,
            decode::ErrorKind::UnknownEntity | decode::ErrorKind::Unsupported => {
                ErrorKind::Unsupported
            }
            _ => ErrorKind::InvalidGeometry,
        },
        stage: Stage::Presentation,
        entity: e.entity,
        source: e.source,
        message: if e.kind == decode::ErrorKind::UnknownEntity {
            outside_profile(
                document,
                &style_profile::SCHEMA_SET,
                "tessstep_style",
                e.entity,
            )
        } else {
            e.to_string()
        },
    };
    match decode(roots) {
        Ok(decoded) => Ok((Some(decoded), Vec::new())),
        Err(e) if e.kind == decode::ErrorKind::ResourceLimit => Err(error(e)),
        Err(_) => {
            let mut kept = Vec::new();
            let mut excluded = Vec::new();
            for &root in roots {
                match decode(&[root]) {
                    Ok(_) => kept.push(root),
                    Err(e) if e.kind == decode::ErrorKind::ResourceLimit => return Err(error(e)),
                    Err(e) => {
                        let e = error(e);
                        excluded.push(ExcludedRecord {
                            record: root,
                            structural: false,
                            error: Error {
                                message: format!("style record #{}: {}", root.get(), e.message),
                                ..e
                            },
                        });
                    }
                }
            }
            if kept.is_empty() {
                return Ok((None, excluded));
            }
            Ok((Some(decode(&kept).map_err(error)?), excluded))
        }
    }
}

fn style_error(kind: ErrorKind, entity: Option<EntityId>, message: &str) -> Error {
    Error {
        kind,
        stage: Stage::Presentation,
        entity,
        source: None,
        message: message.into(),
    }
}
