//! Sphere charts for faces that cover the rest of the sphere around one loop.
//!
//! A spherical face bounded by one loop that does not wind around the chart's axis
//! lies, in that chart, either inside the loop or outside it. Outside means around
//! both poles and the seam (a sphere with a cap cut away), which no rectangle of the
//! chart bounds: trimming would find the loop clockwise. Such a plan is rejected as
//! unsupported in that chart, so that planning moves on to rotated frames; in a frame
//! whose axis passes through the loop, the loop winds once around the axis and the
//! face closes at the far pole with a pole seam and a collapsed edge.
use super::*;

impl Builder {
    /// [`Builder::plan_chart`] for a spherical face, rejecting a plan whose only loop
    /// runs clockwise in the chart.
    pub(super) fn plan_sphere_chart(
        &mut self,
        c: &mut Context<'_, '_>,
        face: &FaceRecord,
        tolerance: f64,
        allow_singular: bool,
    ) -> Result<Planned, Error> {
        let planned = self.plan_chart(c, face, tolerance, allow_singular)?;
        if let (Planned::Face(plan), [only]) = (&planned, face.loops.as_slice()) {
            if only.vertex.is_none()
                && !only.uses.is_empty()
                && plan.holes.is_empty()
                && plan.outer.len() == only.uses.len()
            {
                let chart = self
                    .chart_loop(face, &plan.outer, &plan.pcurves)
                    .map_err(|e| self.pcurve_error(c, plan.outer[0].edge, e))?;
                if chart.winding == [0, 0] && signed_area(&chart.polygon) < 0. {
                    return Err(c.error(
                        ErrorKind::Unsupported,
                        "a spherical face outside its only loop covers both poles of the surface chart",
                    ));
                }
            }
        }
        Ok(planned)
    }
}
