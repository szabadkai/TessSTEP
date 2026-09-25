use tessstep_math::{Affine3, Direction3, ModelSpace, Point3, Vector3};
/// Shared finite, fixed-work exercise for arbitrary IEEE-754 bit patterns.
pub fn exercise(data: &[u8]) {
    let mut values = [0.; 18];
    for (out, bytes) in values.iter_mut().zip(data.chunks_exact(8)) {
        *out = f64::from_bits(u64::from_le_bytes(bytes.try_into().unwrap()));
    }
    type T = Affine3<ModelSpace, ModelSpace>;
    let matrix = std::array::from_fn(|i| std::array::from_fn(|j| values[i * 3 + j]));
    let translation = [values[9], values[10], values[11]];
    let p = [values[12], values[13], values[14]];
    let q = [values[15], values[16], values[17]];
    if let Ok(v) = Vector3::<ModelSpace>::new(p) {
        if let Ok(n) = v.norm() {
            assert!(n.is_finite() && n >= 0.);
        }
        if let Ok(d) = v.normalized() {
            assert!((d.vector().norm().unwrap() - 1.).abs() < 1e-14);
        }
        if let Ok(w) = Vector3::new(q) {
            if let Ok(dot) = v.dot(w) {
                assert!(dot.is_finite());
            }
            if let Ok(cross) = v.cross(w) {
                assert!(cross.components().iter().all(|x| x.is_finite()));
            }
        }
    }
    if let Ok(t) = T::new(matrix, translation) {
        let inverse = t.inverse(Default::default());
        assert_eq!(inverse, t.inverse(Default::default()));
        if let Ok(i) = inverse {
            assert!(
                i.linear()
                    .iter()
                    .flatten()
                    .chain(i.translation().iter())
                    .all(|x| x.is_finite())
            );
        }
        if let Ok(p) = Point3::new(p) {
            if let Ok(p) = t.transform_point(p) {
                assert!(p.coordinates().iter().all(|x| x.is_finite()));
            }
        }
        if let Ok(d) = Direction3::new(q) {
            if let Ok(n) = t.transform_normal(d, Default::default()) {
                assert!((n.vector().norm().unwrap() - 1.).abs() < 1e-14);
            }
            if let Ok(p) = Point3::new(p) {
                let _ = T::from_frame(p, d, d, Default::default());
            }
        }
        let _ = t.then(t);
    }
}
