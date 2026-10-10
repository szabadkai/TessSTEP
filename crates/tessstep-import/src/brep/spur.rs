//! Loop spurs: consecutive edges of a loop that run back along each other.
//!
//! An exporter that bounds a face beside a flush feature can write a loop that runs
//! out along one edge and back along another over the same curve: a slab top whose
//! wall edge spans the whole wall while the base edge of a fin standing flush on that
//! wall spans only its part, or a B-spline that continues the arc it was fitted to and
//! runs back over it first. Between the two edges the loop encloses no area within the
//! working tolerance, and the trimmer rejects it as tangent edges at a vertex
//! (docs/CURVED_IMPORT.md#uv-loop-failures). A tolerant kernel reads the run as
//! collapsed: where the curves part, the edges are split (reusing the vertex already
//! there, or inserting one where the curves cross again), the coincident halves are
//! merged as sliver-face edges are, and the loop's out-and-back run is dropped. No
//! coordinate moves; the neighbouring faces share the surviving edge and the shell
//! stays closed. A tangent cusp, which parts only quadratically, is not a spur.
use super::*;
use crate::pcurve::cross;

/// Where two consecutive uses, `prev` ending at the shared vertex and `next` starting
/// there, part.
enum Cut {
    /// `next` runs within the tolerance band of `prev` along its whole length and its
    /// far vertex lies on `prev`'s curve: split `prev` there, reusing that vertex.
    NextWithinPrev { parameter: f64 },
    /// `prev` runs within the band of `next` along its whole length and its far vertex
    /// lies on `next`'s curve: split `next` there, reusing that vertex.
    PrevWithinNext { parameter: f64 },
    /// Both edges coincide between the same two vertices.
    Whole,
    /// The curves cross again, at an angle, at an interior point of both: split both
    /// there, with one inserted vertex on `prev`'s curve.
    Second {
        prev_parameter: f64,
        next_parameter: f64,
    },
}
struct Spur {
    face: usize,
    prev: Use,
    next: Use,
    cut: Cut,
}
/// One point of an edge walked away from the shared vertex, projected onto the other
/// edge's curve.
struct Sample {
    /// Curve parameter on the walked edge.
    t: f64,
    q: V3,
    /// Parameter of the nearest point on the other curve, and the distance to it.
    foot_t: f64,
    distance: f64,
    /// Offset across the other curve in the surface's tangent plane, when a surface
    /// normal is known: its sign tells the side.
    signed: Option<f64>,
}

/// Curve parameter of `e` at fraction `s` of its range from the end where the shared
/// vertex is.
fn walk_parameter(e: &EdgeRecord, from_start: bool, s: f64) -> f64 {
    let [a, b] = e.range;
    match (from_start, s) {
        (true, 0.) | (false, 1.) => a,
        (true, 1.) | (false, 0.) => b,
        (true, _) => a + s * (b - a),
        (false, _) => b - s * (b - a),
    }
}
fn point(e: &EdgeRecord, t: f64) -> Option<V3> {
    Some(e.curve.evaluate(t).ok()?.position.coordinates())
}
/// The vertex of `u` that the loop reaches first (`start`) or last.
fn vertex_of(e: &EdgeRecord, u: Use, start: bool) -> VertexId {
    e.vertices[usize::from(u.forward != start)]
}
/// Whether `t` lies inside `e`'s range with its point farther than `tolerance` from
/// both ends, so that a split there leaves no sliver half.
fn interior(e: &EdgeRecord, t: f64, tolerance: f64) -> bool {
    let [a, b] = e.range;
    if !(t > a && t < b) {
        return false;
    }
    let (Some(p), Some(pa), Some(pb)) = (point(e, t), point(e, a), point(e, b)) else {
        return false;
    };
    norm(sub(p, pa)) > tolerance && norm(sub(p, pb)) > tolerance
}
/// The nearest point of `e` to `q`, refined by damped Newton steps from the parameter
/// `hint`: (parameter, point, curve tangent there).
fn project(e: &EdgeRecord, q: V3, hint: f64) -> Option<(f64, V3, V3)> {
    let [lo, hi] = e.range;
    let eval = |t: f64| e.curve.evaluate(t).ok();
    let mut t = hint.clamp(lo, hi);
    let mut at = eval(t)?;
    let mut d = norm(sub(at.position.coordinates(), q));
    for _ in 0..32 {
        let r = sub(at.position.coordinates(), q);
        let d1 = at.first.components();
        let f = dot(r, d1);
        let df = dot(d1, d1) + dot(r, at.second.components());
        if !(df.is_finite() && df > 0.) {
            break;
        }
        let mut step = f / df;
        let mut accepted = None;
        for _ in 0..8 {
            let next = (t - step).clamp(lo, hi);
            if next == t {
                break;
            }
            let candidate = eval(next)?;
            let dn = norm(sub(candidate.position.coordinates(), q));
            if dn <= d {
                accepted = Some((next, candidate, dn));
                break;
            }
            step *= 0.5;
        }
        let Some((next, candidate, dn)) = accepted else {
            break;
        };
        let done = (next - t).abs() <= 1e-12 * (hi - lo);
        t = next;
        at = candidate;
        d = dn;
        if done {
            break;
        }
    }
    Some((t, at.position.coordinates(), at.first.components()))
}
/// The unit normal of `face`'s surface at `p`.
fn surface_normal(work: &mut pcurve::Work, face: &FaceRecord, p: V3, tolerance: f64) -> Option<V3> {
    let inverter = Inverter {
        chart: &face.chart,
        surface: &face.surface,
        tolerance,
    };
    let uv = inverter.invert(p, None, work).ok()?;
    let jet = face.surface.evaluate(uv).ok()?;
    let n = cross(jet.du.components(), jet.dv.components());
    let length = norm(n);
    (length > 0. && length.is_finite()).then(|| scale(n, 1. / length))
}
/// Signed offset of `q` from its foot on a curve with tangent `tangent` there, across
/// the curve in the tangent plane of `normal`.
fn side(q: V3, foot: V3, tangent: V3, normal: Option<V3>) -> Option<f64> {
    let n = normal?;
    let length = norm(tangent);
    (length > 0. && length.is_finite())
        .then(|| dot(cross(scale(tangent, 1. / length), sub(q, foot)), n))
}
/// Walk `moving` from the shared vertex (its `from_start` end) in the scan fractions
/// and project each point onto `fixed`, starting at `fixed`'s parameter `hint` there,
/// until a point leaves the tolerance band. Returns the points within the band and
/// whether the whole edge stayed within it; `None` when a curve fails to evaluate.
fn walk(
    c: &mut Context<'_, '_>,
    moving: &EdgeRecord,
    from_start: bool,
    fixed: &EdgeRecord,
    hint: f64,
    normal: Option<V3>,
    tolerance: f64,
) -> Result<Option<(Vec<Sample>, bool)>, Error> {
    let mut samples = Vec::new();
    let mut foot_t = hint;
    for s in scan_parameters([0., 1.]) {
        c.charge(1)?;
        let t = walk_parameter(moving, from_start, s);
        let Some(q) = point(moving, t) else {
            return Ok(None);
        };
        let Some((ft, foot, tangent)) = project(fixed, q, foot_t) else {
            return Ok(None);
        };
        let distance = norm(sub(q, foot));
        if distance.is_nan() || distance > tolerance {
            return Ok(Some((samples, false)));
        }
        foot_t = ft;
        samples.push(Sample {
            t,
            q,
            foot_t: ft,
            distance,
            signed: side(q, foot, tangent, normal),
        });
    }
    Ok(Some((samples, true)))
}

impl Builder {
    /// Whether some loop has a spur at `tolerance` (module documentation); strict
    /// import keeps every loop as written.
    pub(super) fn has_spur(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &[FaceRecord],
        tolerance: f64,
    ) -> Result<bool, Error> {
        if self.strict {
            return Ok(false);
        }
        self.work.remaining = self.pass_work;
        Ok(self.find(c, faces, 0, tolerance)?.is_some())
    }
    /// Collapse every loop spur. Each found spur is healed before the search resumes,
    /// since healing renumbers edges; faces already searched cannot gain a spur from
    /// a later heal, which only splits their edges or drops a run from another loop.
    pub(super) fn collapse_spurs(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut [FaceRecord],
        tolerance: f64,
    ) -> Result<(), Error> {
        if self.strict {
            return Ok(());
        }
        self.work.remaining = self.pass_work;
        let mut start = 0;
        for _ in 0..256 {
            let Some(spur) = self.find(c, faces, start, tolerance)? else {
                return Ok(());
            };
            start = spur.face;
            self.heal(c, faces, spur)?;
        }
        Ok(())
    }
    fn find(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &[FaceRecord],
        start: usize,
        tolerance: f64,
    ) -> Result<Option<Spur>, Error> {
        for (index, face) in faces.iter().enumerate().skip(start) {
            for l in &face.loops {
                let n = l.uses.len();
                if n < 2 {
                    continue;
                }
                for i in 0..n {
                    let (prev, next) = (l.uses[i], l.uses[(i + 1) % n]);
                    c.charge(1)?;
                    if let Some(cut) = self.cut(c, face, prev, next, tolerance)? {
                        // Healing drops the two uses and adds one per split edge of
                        // this loop; a loop left with fewer than three uses is a
                        // sliver face, not a face with a spur, and stays as written.
                        let kept = n + match cut {
                            Cut::NextWithinPrev { .. } | Cut::PrevWithinNext { .. } => 1,
                            Cut::Second { .. } => 2,
                            Cut::Whole => 0,
                        } - 2;
                        if kept < 3 {
                            continue;
                        }
                        // The merged half beside the vertex survives only through the
                        // other uses of the two edges: on an open shell a spur of free
                        // edges would leave it a record that no loop uses, so such a
                        // loop stays as written.
                        c.charge(faces.len())?;
                        let uses = |edge: usize| {
                            faces
                                .iter()
                                .flat_map(|f| &f.loops)
                                .flat_map(|l| &l.uses)
                                .filter(|u| u.edge == edge)
                                .count()
                        };
                        if uses(prev.edge) + uses(next.edge) <= 2 {
                            continue;
                        }
                        return Ok(Some(Spur {
                            face: index,
                            prev,
                            next,
                            cut,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }
    /// Where `prev` and `next`, meeting at a vertex, part again, or `None` when they
    /// form an ordinary corner (or no decision can be made).
    fn cut(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        prev: Use,
        next: Use,
        tolerance: f64,
    ) -> Result<Option<Cut>, Error> {
        if prev.edge == next.edge {
            return Ok(None);
        }
        let (p, n) = (&self.edges[prev.edge], &self.edges[next.edge]);
        if p.vertices[0] == p.vertices[1] || n.vertices[0] == n.vertices[1] {
            return Ok(None);
        }
        let v = vertex_of(p, prev, false);
        if vertex_of(n, next, true) != v {
            return Ok(None);
        }
        let vp = c.raw.vertices[v.0].position.coordinates();
        // The shared vertex is at `prev`'s end when the use is forward, and at
        // `next`'s start.
        let (p_from_start, n_from_start) = (!prev.forward, next.forward);
        let p_at_v = walk_parameter(p, p_from_start, 0.);
        let n_at_v = walk_parameter(n, n_from_start, 0.);
        // An ordinary corner parts within a few tolerances of the vertex: a point of
        // `next` two tolerances out that lies off `prev`'s tangent line there by more
        // than three is no spur. Vertices and curve ends may each sit a tolerance apart.
        let Some(p_end) = p.curve.evaluate(p_at_v).ok() else {
            return Ok(None);
        };
        let (origin, tangent) = (p_end.position.coordinates(), p_end.first.components());
        let length = norm(tangent);
        if !(length > 0. && length.is_finite()) {
            return Ok(None);
        }
        let direction = scale(tangent, 1. / length);
        for i in (1..=16).rev() {
            c.charge(1)?;
            let Some(q) = point(n, walk_parameter(n, n_from_start, 0.5f64.powi(i))) else {
                return Ok(None);
            };
            if norm(sub(q, vp)) < 2. * tolerance {
                continue;
            }
            let r = sub(q, origin);
            let off = sub(r, scale(direction, dot(r, direction)));
            if norm(off) > 3. * tolerance {
                return Ok(None);
            }
            break;
        }
        let normal = surface_normal(&mut self.work, face, vp, tolerance);
        let (p, n) = (&self.edges[prev.edge], &self.edges[next.edge]);
        let far = |e: &EdgeRecord, u: Use, start: bool| {
            let w = vertex_of(e, u, start);
            (w, c.raw.vertices[w.0].position.coordinates())
        };
        let (p_far, p_far_point) = far(p, prev, true);
        let (n_far, n_far_point) = far(n, next, false);
        // Whether `e`'s far vertex lies on `other`, nearest from the parameter `hint`:
        // its parameter there and its distance.
        let on = |other: &EdgeRecord, far_point: V3, hint: f64| -> Option<(f64, f64)> {
            let (t, foot, _) = project(other, far_point, hint)?;
            let distance = norm(sub(foot, far_point));
            (distance <= tolerance).then_some((t, distance))
        };
        // A spur is a lobe much longer than wide: its run from the vertex spans
        // several tolerances (a corner of a body whose edges are a few tolerances
        // long lies within the band whatever its angle, and is left as written) and
        // the curves stay within a tenth of that run of each other (a corner that
        // opens at a few degrees is a corner).
        let thin = |run: f64, width: f64| run >= 4. * tolerance && width <= 0.1 * run;
        let widest = |samples: &[Sample]| samples.iter().map(|s| s.distance).fold(0., f64::max);
        let Some((samples, complete)) = walk(c, n, n_from_start, p, p_at_v, normal, tolerance)?
        else {
            return Ok(None);
        };
        if complete {
            let last = samples.last().map_or(p_at_v, |s| s.foot_t);
            if let Some((t, distance)) = on(p, n_far_point, last) {
                if !thin(norm(sub(n_far_point, vp)), widest(&samples).max(distance)) {
                    return Ok(None);
                }
                if p_far == n_far {
                    return Ok(Some(Cut::Whole));
                }
                // Two distinct far vertices on each other's curves need a vertex
                // merge, which only the opt-in sewing repair makes.
                if on(n, p_far_point, walk_parameter(n, n_from_start, 1.)).is_some() {
                    return Ok(None);
                }
                return Ok(
                    interior(p, t, tolerance).then_some(Cut::NextWithinPrev { parameter: t })
                );
            }
        }
        let Some((p_samples, p_complete)) = walk(c, p, p_from_start, n, n_at_v, normal, tolerance)?
        else {
            return Ok(None);
        };
        if p_complete {
            let last = p_samples.last().map_or(n_at_v, |s| s.foot_t);
            if let Some((t, distance)) = on(n, p_far_point, last) {
                if !thin(norm(sub(p_far_point, vp)), widest(&p_samples).max(distance)) {
                    return Ok(None);
                }
                if p_far == n_far {
                    return Ok(Some(Cut::Whole));
                }
                if interior(n, t, tolerance) {
                    return Ok(Some(Cut::PrevWithinNext { parameter: t }));
                }
                return Ok(None);
            }
        }
        // A second crossing of `next` over `prev` beyond the tolerance box around the
        // vertex, where the trimmer's noise allowance ends. The curves must cross at
        // an angle: a tangent cusp (an arc and its tangent line, which part only
        // quadratically) and the fit noise along it are no spur, however long they
        // stay within the tolerance.
        let eps = (tolerance * 1e-3).max(1e-12);
        // The last sample clearly on one side; a sample on the curve itself (within
        // the touch threshold) decides nothing and may be the crossing.
        let mut last = None;
        for (j, s) in samples.iter().enumerate() {
            let Some(now) = s.signed else {
                continue;
            };
            if now.abs() <= eps {
                continue;
            }
            let Some(k) = last.replace(j) else {
                continue;
            };
            if norm(sub(s.q, vp)) <= tolerance {
                continue;
            }
            let before: &Sample = &samples[k];
            let was = before.signed.expect("a clearly sided sample");
            if was * now >= 0. || (now - was).abs() <= 1e-3 * norm(sub(s.q, before.q)) {
                continue;
            }
            // Bisect the crossing between the two samples.
            let (mut lo, mut hi) = (before.t, s.t);
            let (mut lo_side, mut foot_t) = (was, before.foot_t);
            for _ in 0..48 {
                c.charge(1)?;
                let mid = 0.5 * (lo + hi);
                let Some(q) = point(n, mid) else {
                    return Ok(None);
                };
                let Some((ft, foot, tangent)) = project(p, q, foot_t) else {
                    return Ok(None);
                };
                foot_t = ft;
                let Some(m) = side(q, foot, tangent, normal) else {
                    return Ok(None);
                };
                if m.abs() <= eps {
                    lo = mid;
                    break;
                }
                if (m < 0.) == (lo_side < 0.) {
                    lo = mid;
                    lo_side = m;
                } else {
                    hi = mid;
                }
            }
            let t = lo;
            let Some(x) = point(n, t) else {
                return Ok(None);
            };
            let Some((pt, foot, _)) = project(p, x, s.foot_t) else {
                return Ok(None);
            };
            if !thin(norm(sub(x, vp)), widest(&samples[..=j]))
                || norm(sub(foot, vp)) <= tolerance
                || !interior(n, t, tolerance)
                || !interior(p, pt, tolerance)
                || norm(sub(x, n_far_point)) <= tolerance
                || norm(sub(foot, p_far_point)) <= tolerance
            {
                return Ok(None);
            }
            return Ok(Some(Cut::Second {
                prev_parameter: pt,
                next_parameter: t,
            }));
        }
        Ok(None)
    }
    /// Split the edges where they part, merge the coincident halves beside the shared
    /// vertex and drop the loop's out-and-back run.
    fn heal(
        &mut self,
        c: &mut Context<'_, '_>,
        faces: &mut [FaceRecord],
        spur: Spur,
    ) -> Result<(), Error> {
        let Spur {
            face,
            prev,
            next,
            cut,
        } = spur;
        c.current = faces[face].entity;
        let (p, n) = (prev.edge, next.edge);
        // After `split`, the half beside the shared vertex keeps the edge's index when
        // the vertex is at the range start, and is the appended edge otherwise.
        let inner = |edges: &[EdgeRecord], edge: usize, from_start: bool| {
            if from_start { edge } else { edges.len() - 1 }
        };
        let (removed, survivor) = match cut {
            Cut::NextWithinPrev { parameter } => {
                let w = vertex_of(&self.edges[n], next, false);
                self.split(c, faces, p, parameter, Some(w))?;
                (n, inner(&self.edges, p, !prev.forward))
            }
            Cut::PrevWithinNext { parameter } => {
                let w = vertex_of(&self.edges[p], prev, true);
                self.split(c, faces, n, parameter, Some(w))?;
                (inner(&self.edges, n, next.forward), p)
            }
            Cut::Whole => (n, p),
            Cut::Second {
                prev_parameter,
                next_parameter,
            } => {
                self.split(c, faces, p, prev_parameter, None)?;
                let appended = &self.edges[self.edges.len() - 1];
                let vertex = appended.vertices[0];
                let p_inner = inner(&self.edges, p, !prev.forward);
                self.split(c, faces, n, next_parameter, Some(vertex))?;
                (inner(&self.edges, n, next.forward), p_inner)
            }
        };
        // An analytic edge survives a B-spline, as for sliver faces.
        let (removed, survivor) = if matches!(self.edges[survivor].shape, CurveShape::Nurbs)
            && !matches!(self.edges[removed].shape, CurveShape::Nurbs)
        {
            (survivor, removed)
        } else {
            (removed, survivor)
        };
        let (rv, sv) = (self.edges[removed].vertices, self.edges[survivor].vertices);
        let flip = rv != sv;
        debug_assert!(!flip || rv == [sv[1], sv[0]]);
        c.record()?;
        self.retarget(faces, &BTreeMap::from([(removed, (survivor, flip))]));
        let survivor = self.edge_index_of(survivor, removed);
        for face in faces.iter_mut() {
            for l in &mut face.loops {
                loop {
                    let count = l.uses.len();
                    if count < 3 {
                        break;
                    }
                    c.charge(count)?;
                    let found = (0..count).find(|&i| {
                        let (a, b) = (l.uses[i], l.uses[(i + 1) % count]);
                        a.edge == survivor && b.edge == survivor && a.forward != b.forward
                    });
                    let Some(i) = found else {
                        break;
                    };
                    let j = (i + 1) % count;
                    l.uses.remove(i.max(j));
                    l.uses.remove(i.min(j));
                }
            }
        }
        self.adaptations.collapsed_spurs += 1;
        Ok(())
    }
    /// The compacted index of `survivor` after `removed` left the edge table.
    fn edge_index_of(&self, survivor: usize, removed: usize) -> usize {
        if survivor > removed {
            survivor - 1
        } else {
            survivor
        }
    }
}
