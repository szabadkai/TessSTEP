//! Swept surfaces (`SURFACE_OF_LINEAR_EXTRUSION`, `SURFACE_OF_REVOLUTION`).
//!
//! Special cases become the elementary surface they are exactly: an extruded line is
//! a plane, an extruded circle along its normal a cylinder, a revolved line a
//! cylinder, cone or plane, and a revolved circle in a plane through the axis a torus
//! or sphere. Every other swept curve is converted exactly to NURBS: extrusions are
//! degree 1 across the sweep, revolutions use the rational quadratic full circle.
//! The chart's normal is compared with the STEP normal (`C'(u) x V` for an extrusion,
//! `(a x (C - A)) x C'(v)` for a revolution at angle zero) and the face orientation is
//! flipped when they disagree, so geometry and the face's outward side are unchanged.
use super::*;
use crate::pcurve::{Inverter, cross, surface_jet};
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

/// Direction agreement below which two unit vectors count as parallel or orthogonal.
const ANGLE: f64 = 1e-9;

fn unit(v: V3) -> Option<V3> {
    let n = norm(v);
    (n > 0. && n.is_finite()).then(|| scale(v, 1. / n))
}

/// A TRIMMED_CURVE's basis curve and whether it runs in the trimmed curve's sense.
/// Edge vertices bound every use, so the trimming points are not read.
pub(super) fn trimmed_basis<'d, 'a>(
    c: &mut Context<'d, 'a>,
    mut v: &'d EntityView<'a>,
) -> Result<(&'d EntityView<'a>, bool), Error> {
    let mut sense = true;
    for _ in 0..8 {
        if !c.is(v, "TRIMMED_CURVE") {
            return Ok((v, sense));
        }
        c.charge(1)?;
        c.current = v.id;
        sense ^= !c.boolean(v, "SENSE_AGREEMENT")?;
        v = c.reference(v, "BASIS_CURVE")?;
    }
    Err(c.error(ErrorKind::Unsupported, "TRIMMED_CURVE nesting is too deep"))
}

fn plane_frame(
    c: &Context<'_, '_>,
    o: V3,
    x: V3,
    y: V3,
) -> Result<PlaneFrame<ModelSpace, 3>, Error> {
    c.checked(PlaneFrame::new(
        c.checked(Point::new(o))?,
        c.checked(Vector::new(x))?,
        c.checked(Vector::new(y))?,
        NumericalTolerance::default(),
    ))
}

/// A unit vector perpendicular to `z`.
fn perpendicular(z: V3) -> V3 {
    let helper = if z[0].abs() < 0.9 {
        [1., 0., 0.]
    } else {
        [0., 1., 0.]
    };
    let p = cross(z, helper);
    scale(p, 1. / norm(p))
}

/// Exact NURBS of a full conic: nine rational quadratic controls, one quarter per span.
fn conic_nurbs(frame: &Frame, a: f64, b: f64) -> Option<NurbsCurve<ModelSpace, 3>> {
    let corners = [
        (1., 0.),
        (1., 1.),
        (0., 1.),
        (-1., 1.),
        (-1., 0.),
        (-1., -1.),
        (0., -1.),
        (1., -1.),
        (1., 0.),
    ];
    let controls: Option<Vec<_>> = corners
        .iter()
        .map(|&(x, y)| {
            Point::new(add(
                frame.o,
                add(scale(frame.x, a * x), scale(frame.y, b * y)),
            ))
            .ok()
        })
        .collect();
    let weights: Vec<f64> = (0..9)
        .map(|i| if i % 2 == 0 { 1. } else { FRAC_1_SQRT_2 })
        .collect();
    NurbsCurve::new(
        2,
        &full_circle_knots(),
        &controls?,
        &weights,
        SplineLimits::default(),
    )
    .ok()
}
fn full_circle_knots() -> [f64; 12] {
    let q = FRAC_PI_2;
    [
        0.,
        0.,
        0.,
        q,
        q,
        2. * q,
        2. * q,
        3. * q,
        3. * q,
        4. * q,
        4. * q,
        4. * q,
    ]
}

/// NURBS form of a non-line swept curve.
fn swept_nurbs(
    c: &Context<'_, '_>,
    curve: &CurveGeometry<ModelSpace, 3>,
    shape: &CurveShape,
) -> Result<NurbsCurve<ModelSpace, 3>, Error> {
    match (curve, shape) {
        (CurveGeometry::Nurbs(n), _) => Ok(n.clone()),
        (_, CurveShape::Conic { frame, a, b }) => conic_nurbs(frame, *a, *b)
            .ok_or_else(|| c.error(ErrorKind::InvalidGeometry, "conic has no NURBS form")),
        _ => Err(c.error(
            ErrorKind::Unsupported,
            "swept curve has no bounded NURBS form",
        )),
    }
}

fn nurbs_chart(
    c: &Context<'_, '_>,
    surface: NurbsSurface<ModelSpace>,
    closed: [bool; 2],
) -> Result<(Chart, SurfaceGeometry), Error> {
    let tolerance = c.tolerance.distance().as_metres();
    let mut surface = surface;
    let mut periods = [None; 2];
    for axis in 0..2 {
        if !closed[axis] {
            continue;
        }
        let mut axes = surface.periodic_axes();
        axes[axis] = true;
        if let Ok(periodic) = surface.clone().with_periodic_axes(axes, tolerance) {
            let [lo, hi] = periodic.knot_vectors()[axis].domain();
            periods[axis] = Some(hi - lo);
            surface = periodic;
        }
    }
    let singular = pcurve::nurbs_singular(&surface, tolerance);
    Ok((
        Chart::Nurbs(periods, singular),
        SurfaceGeometry::Nurbs(surface),
    ))
}

fn closed_curve(curve: &NurbsCurve<ModelSpace, 3>, tolerance: f64) -> bool {
    let [lo, hi] = curve.knot_vector().domain();
    match (curve.evaluate(lo), curve.evaluate(hi)) {
        (Ok(a), Ok(b)) => a
            .position
            .distance(b.position)
            .is_ok_and(|d| d <= tolerance),
        _ => false,
    }
}

impl Builder {
    /// The chart, kernel surface and whether its normal opposes the STEP normal.
    /// `points` sample the face boundary; they size unbounded sweeps (extrusions and
    /// revolved lines) and select a cone's nappe.
    pub(super) fn swept_surface<'d, 'a>(
        &mut self,
        c: &mut Context<'d, 'a>,
        v: &'d EntityView<'a>,
        points: &[V3],
    ) -> Result<(Chart, SurfaceGeometry, bool), Error> {
        let tolerance = c.tolerance.distance().as_metres();
        let swept = c.reference(v, "SWEPT_CURVE")?;
        let (basis, sense) = trimmed_basis(c, swept)?;
        let (curve, shape) = self.curve(c, basis)?;
        c.current = v.id;
        if points.is_empty() {
            return Err(c.error(ErrorKind::InvalidGeometry, "swept face has no boundary"));
        }
        let sign = if sense { 1. } else { -1. };
        // Curve parameters at which the STEP normal is sampled.
        let parameters: Vec<f64> = match (&curve, &shape) {
            (CurveGeometry::Nurbs(n), _) => {
                let [lo, hi] = n.knot_vector().domain();
                (0..=8).map(|i| lo + (hi - lo) * i as f64 / 8.).collect()
            }
            (_, CurveShape::Conic { .. }) => (0..8).map(|i| PI * i as f64 / 4.).collect(),
            (_, CurveShape::Line { origin, tangent }) => points
                .iter()
                .map(|p| dot(sub(*p, *origin), *tangent) / dot(*tangent, *tangent))
                .collect(),
            _ => Vec::new(),
        };
        let jets: Vec<(V3, V3)> = parameters
            .iter()
            .filter_map(|&t| curve.evaluate(t).ok())
            .map(|e| (e.position.coordinates(), e.first.components()))
            .collect();
        let (chart, surface, samples) = if c.is(v, "SURFACE_OF_LINEAR_EXTRUSION") {
            let vector = c.reference(v, "EXTRUSION_AXIS")?;
            let axis = c
                .direction(c.reference(vector, "ORIENTATION")?)?
                .components();
            if c.real(vector, "MAGNITUDE")? <= 0. {
                return Err(c.error(
                    ErrorKind::InvalidGeometry,
                    "extrusion axis needs a positive magnitude",
                ));
            }
            let (chart, surface, offset) = self.extrusion(c, &curve, &shape, axis, points)?;
            let samples: Vec<(V3, V3)> = jets
                .iter()
                .map(|&(p, d)| (add(p, scale(axis, offset)), scale(cross(d, axis), sign)))
                .collect();
            (chart, surface, samples)
        } else {
            let placement = c.reference(v, "AXIS_POSITION")?;
            let origin = c.point(c.reference(placement, "LOCATION")?)?.coordinates();
            let attribute = c.attr(placement, "AXIS")?;
            let axis = if matches!(attribute.kind, ValueKind::Null) {
                [0., 0., 1.]
            } else {
                c.direction(c.target(attribute)?)?.components()
            };
            c.current = v.id;
            let (chart, surface) = self.revolution(c, &curve, &shape, origin, axis, points)?;
            let samples: Vec<(V3, V3)> = jets
                .iter()
                .map(|&(p, d)| (p, scale(cross(cross(axis, sub(p, origin)), d), sign)))
                .collect();
            (chart, surface, samples)
        };
        // Compare the STEP normal with the chart's at the first well-conditioned sample.
        let inverter = Inverter {
            chart: &chart,
            surface: &surface,
            tolerance,
        };
        for (p, n) in samples {
            let Some(n) = unit(n) else {
                continue;
            };
            let Ok(uv) = inverter.invert(p, None, &mut self.work) else {
                continue;
            };
            let Ok(jet) = surface_jet(&surface, uv, &mut self.work) else {
                continue;
            };
            if norm(sub(jet.p, p)) > 10. * tolerance {
                continue;
            }
            let Some(m) = unit(cross(jet.du, jet.dv)) else {
                continue;
            };
            if dot(m, n).abs() < 0.5 {
                continue;
            }
            return Ok((chart, surface, dot(m, n) < 0.));
        }
        Err(c.error(
            ErrorKind::InvalidGeometry,
            "swept surface normal is degenerate at every sampled point",
        ))
    }

    /// Extrusion chart and the sweep offset of its normal samples along the axis.
    fn extrusion(
        &mut self,
        c: &Context<'_, '_>,
        curve: &CurveGeometry<ModelSpace, 3>,
        shape: &CurveShape,
        axis: V3,
        points: &[V3],
    ) -> Result<(Chart, SurfaceGeometry, f64), Error> {
        let tolerance = c.tolerance.distance().as_metres();
        match shape {
            CurveShape::Line { origin, tangent } => {
                let x = unit(*tangent)
                    .ok_or_else(|| c.error(ErrorKind::InvalidGeometry, "zero line direction"))?;
                let y = unit(sub(axis, scale(x, dot(axis, x))))
                    .filter(|_| norm(cross(x, axis)) > ANGLE)
                    .ok_or_else(|| {
                        c.error(
                            ErrorKind::InvalidGeometry,
                            "extrusion axis is parallel to its line",
                        )
                    })?;
                let frame = plane_frame(c, *origin, x, y)?;
                return Ok((
                    Chart::Plane(Frame::new(frame)),
                    SurfaceGeometry::Analytic(Surface::plane(frame)),
                    0.,
                ));
            }
            CurveShape::Conic { frame, a, b } if a == b && norm(cross(frame.z, axis)) <= ANGLE => {
                let placed = plane_frame(c, frame.o, frame.x, frame.y)?;
                let surface = Surface::cylinder(placed, c.checked(Length::metres(*a))?)
                    .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
                return Ok((
                    Chart::Cylinder(Frame::new(placed), *a),
                    SurfaceGeometry::Analytic(surface),
                    0.,
                ));
            }
            _ => {}
        }
        let base = swept_nurbs(c, curve, shape)?;
        // Sweep offsets covering the face: every boundary point is C(u) + s * axis.
        let (pmin, pmax) = points
            .iter()
            .map(|p| dot(*p, axis))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| {
                (lo.min(s), hi.max(s))
            });
        let (cmin, cmax) = base
            .controls()
            .iter()
            .map(|p| dot(p.coordinates(), axis))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), s| {
                (lo.min(s), hi.max(s))
            });
        let (mut s0, mut s1) = (pmin - cmax, pmax - cmin);
        let margin = (1e-3 * (s1 - s0)).max(10. * tolerance);
        s0 -= margin;
        s1 += margin;
        let mut controls = Vec::with_capacity(2 * base.controls().len());
        let mut weights = Vec::with_capacity(controls.capacity());
        for (p, &w) in base.controls().iter().zip(base.weights()) {
            for s in [s0, s1] {
                controls.push(c.checked(Point::new(add(p.coordinates(), scale(axis, s))))?);
                weights.push(w);
            }
        }
        let surface = NurbsSurface::new(
            [base.knot_vector().degree(), 1],
            [base.knot_vector().knots(), &[s0, s0, s1, s1]],
            [base.controls().len(), 2],
            &controls,
            &weights,
            SplineLimits::default(),
        )
        .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
        let (chart, surface) = nurbs_chart(c, surface, [closed_curve(&base, tolerance), false])?;
        Ok((chart, surface, 0.5 * (s0 + s1)))
    }

    fn revolution(
        &mut self,
        c: &Context<'_, '_>,
        curve: &CurveGeometry<ModelSpace, 3>,
        shape: &CurveShape,
        origin: V3,
        axis: V3,
        points: &[V3],
    ) -> Result<(Chart, SurfaceGeometry), Error> {
        let tolerance = c.tolerance.distance().as_metres();
        let radial = |p: V3| {
            let w = sub(p, origin);
            sub(w, scale(axis, dot(w, axis)))
        };
        let shape_error = |e: tessstep_surfaces::Error| c.error(ErrorKind::InvalidGeometry, e);
        let base = match shape {
            CurveShape::Line {
                origin: l0,
                tangent,
            } => {
                let d = unit(*tangent)
                    .ok_or_else(|| c.error(ErrorKind::InvalidGeometry, "zero line direction"))?;
                let skew = norm(cross(d, axis));
                if skew <= ANGLE {
                    // A line parallel to the axis sweeps a cylinder.
                    let r = radial(*l0);
                    let radius = norm(r);
                    if radius <= tolerance {
                        return Err(
                            c.error(ErrorKind::InvalidGeometry, "revolved line lies on its axis")
                        );
                    }
                    let x = scale(r, 1. / radius);
                    let frame = plane_frame(c, origin, x, cross(axis, x))?;
                    return Ok((
                        Chart::Cylinder(Frame::new(frame), radius),
                        SurfaceGeometry::Analytic(
                            Surface::cylinder(frame, c.checked(Length::metres(radius))?)
                                .map_err(shape_error)?,
                        ),
                    ));
                }
                if dot(d, axis).abs() <= ANGLE {
                    // A line perpendicular to the axis sweeps a plane.
                    let frame = plane_frame(c, *l0, d, cross(axis, d))?;
                    return Ok((
                        Chart::Plane(Frame::new(frame)),
                        SurfaceGeometry::Analytic(Surface::plane(frame)),
                    ));
                }
                let w = sub(*l0, origin);
                let b = dot(d, axis);
                if dot(w, cross(d, axis)).abs() / skew <= tolerance {
                    // A line meeting the axis sweeps a cone with its apex there.
                    let s = (dot(w, axis) - b * dot(w, d)) / (1. - b * b);
                    let apex = add(origin, scale(axis, s));
                    let side: Vec<f64> = points.iter().map(|p| dot(sub(*p, apex), axis)).collect();
                    let above = side.iter().filter(|&&x| x > tolerance).count();
                    let below = side.iter().filter(|&&x| x < -tolerance).count();
                    if above > 0 && below > 0 {
                        return Err(c.error(
                            ErrorKind::Unsupported,
                            "revolved line crosses its axis within the face",
                        ));
                    }
                    let z = if below > 0 { scale(axis, -1.) } else { axis };
                    let x = points
                        .iter()
                        .filter_map(|p| unit(radial(*p)).filter(|_| norm(radial(*p)) > tolerance))
                        .next()
                        .unwrap_or_else(|| perpendicular(z));
                    let x = unit(sub(x, scale(z, dot(x, z)))).unwrap_or_else(|| perpendicular(z));
                    let frame = plane_frame(c, apex, x, cross(z, x))?;
                    let alpha = b.abs().clamp(-1., 1.).acos();
                    return Ok((
                        Chart::Cone(Frame::new(frame), alpha),
                        SurfaceGeometry::Analytic(
                            Surface::cone(frame, c.checked(Angle::radians(alpha))?)
                                .map_err(shape_error)?,
                        ),
                    ));
                }
                // A skew line sweeps a hyperboloid: a segment covering the face's
                // axial range is revolved as NURBS.
                let (lo, hi) = points
                    .iter()
                    .map(|p| (dot(sub(*p, origin), axis) - dot(w, axis)) / b)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
                        (lo.min(t), hi.max(t))
                    });
                let margin = (1e-3 * (hi - lo)).max(10. * tolerance);
                let ends = [lo - margin, hi + margin].map(|t| Point::new(add(*l0, scale(d, t))));
                let [Ok(a), Ok(b)] = ends else {
                    return Err(c.error(ErrorKind::InvalidGeometry, "revolved line is unbounded"));
                };
                NurbsCurve::new(
                    1,
                    &[0., 0., 1., 1.],
                    &[a, b],
                    &[1.; 2],
                    SplineLimits::default(),
                )
                .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?
            }
            CurveShape::Conic { frame, a, b }
                if a == b
                    && dot(frame.z, axis).abs() <= ANGLE
                    && dot(sub(origin, frame.o), frame.z).abs() <= tolerance =>
            {
                // A circle in a plane through the axis sweeps a sphere or a torus.
                let r = radial(frame.o);
                let major = norm(r);
                let centre = sub(frame.o, r);
                if major <= tolerance {
                    let x = perpendicular(axis);
                    let placed = plane_frame(c, centre, x, cross(axis, x))?;
                    return Ok((
                        Chart::Sphere(Frame::new(placed), *a),
                        SurfaceGeometry::Analytic(
                            Surface::sphere(placed, c.checked(Length::metres(*a))?)
                                .map_err(shape_error)?,
                        ),
                    ));
                }
                if major > *a {
                    let x = scale(r, 1. / major);
                    let placed = plane_frame(c, centre, x, cross(axis, x))?;
                    return Ok((
                        Chart::Torus(Frame::new(placed), major, *a),
                        SurfaceGeometry::Analytic(
                            Surface::torus(
                                placed,
                                c.checked(Length::metres(major))?,
                                c.checked(Length::metres(*a))?,
                            )
                            .map_err(shape_error)?,
                        ),
                    ));
                }
                swept_nurbs(c, curve, shape)?
            }
            _ => swept_nurbs(c, curve, shape)?,
        };
        // Revolve the NURBS profile: nine rational quadratic controls per profile control.
        let knots = full_circle_knots();
        let nv = base.controls().len();
        let mut controls = vec![None; 9 * nv];
        let mut weights = vec![0.; 9 * nv];
        for (j, (q, &w)) in base.controls().iter().zip(base.weights()).enumerate() {
            let q = q.coordinates();
            let r = radial(q);
            let radius = norm(r);
            let centre = sub(q, r);
            let x = if radius > 0. {
                scale(r, 1. / radius)
            } else {
                perpendicular(axis)
            };
            let y = cross(axis, x);
            for i in 0..9 {
                let angle = FRAC_PI_2 * 0.5 * i as f64;
                let (scale_i, weight) = if i % 2 == 0 {
                    (1., 1.)
                } else {
                    (std::f64::consts::SQRT_2, FRAC_1_SQRT_2)
                };
                let p = add(
                    centre,
                    scale(
                        add(scale(x, angle.cos()), scale(y, angle.sin())),
                        radius * scale_i,
                    ),
                );
                controls[i * nv + j] = Some(c.checked(Point::new(p))?);
                weights[i * nv + j] = weight * w;
            }
        }
        let controls: Vec<_> = controls.into_iter().map(|p| p.expect("filled")).collect();
        let surface = NurbsSurface::new(
            [2, base.knot_vector().degree()],
            [&knots, base.knot_vector().knots()],
            [9, nv],
            &controls,
            &weights,
            SplineLimits::default(),
        )
        .map_err(|e| c.error(ErrorKind::InvalidGeometry, e))?;
        nurbs_chart(c, surface, [true, closed_curve(&base, tolerance)])
    }
}
