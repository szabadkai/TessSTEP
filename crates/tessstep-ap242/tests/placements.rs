use std::collections::BTreeMap;
use tessstep_ap242::{ErrorKind, Limits, adapt};
use tessstep_math::{ModelSpace, Point3};
use tessstep_model::{decode, parse};
use tessstep_product::*;
#[allow(dead_code)]
mod schema {
    include!("support/schema.rs");
}
const INPUT: &str = include_str!("../../../corpus/product/assembly.step");
fn run(input: &str) -> Result<Model, tessstep_ap242::Error> {
    let doc = parse(input.as_bytes(), Default::default()).unwrap();
    let decoded = decode::decode(
        &doc,
        &schema::SCHEMA_SET,
        "product_test",
        Default::default(),
    )
    .unwrap();
    adapt(&decoded, "product_test", Limits::default())
}
fn extra(input: &str, body: &str) -> String {
    input.replace("ENDSEC;\nEND-ISO", &format!("{body}\nENDSEC;\nEND-ISO"))
}
fn close(a: [f64; 3], b: [f64; 3]) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < 1e-12, "{a:?} != {b:?}");
    }
}
fn point(t: Transform, p: [f64; 3]) -> [f64; 3] {
    t.transform_point(Point3::<ModelSpace>::new(p).unwrap())
        .unwrap()
        .coordinates()
}
#[test]
fn product_placements_normalize_units_and_preserve_direction() {
    let model = run(INPUT).unwrap();
    let left = model.parts().relationship_transforms[0].rep_1_to_rep_2;
    close(point(left, [0.0254, 0., 0.]), [0.1, 0., 0.]);
    close(point(left, [1.0254, 0., 0.]), [1.1, 0., 0.]);
    let instances = model
        .expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(instances.len(), 3);
    assert_eq!(instances[1].parent, Some(0));
    assert_eq!(instances[1].occurrence, Some(OccurrenceId(7)));
    assert_eq!(instances[2].occurrence, Some(OccurrenceId(8)));
    close(
        point(instances[2].local_to_world, [0.0254, 0., 0.]),
        [0., 0.2, 0.],
    );
    let reversed = INPUT.replace("(#35,#34,#40)", "(#34,#35,#40)").replace(
        "ITEM_DEFINED_TRANSFORMATION(#32,#30)",
        "ITEM_DEFINED_TRANSFORMATION(#30,#32)",
    );
    let inverse = run(&reversed)
        .unwrap()
        .expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default(),
        )
        .unwrap();
    close(
        point(inverse[1].local_to_world, [0.0254, 0., 0.]),
        [0.1, 0., 0.],
    );
}
#[test]
fn product_axis_defaults_rotation_and_degenerate_directions() {
    let input = extra(
        &INPUT.replace("placement',#70,$,$)", "placement',#70,#80,$)"),
        "#80=DIRECTION('',(1.,0.,0.));",
    );
    let model = run(&input).unwrap();
    let t = model.parts().relationship_transforms[0].rep_1_to_rep_2;
    close(point(t, [1.0254, 0., 0.]), [0.1, 1., 0.]);
    for ratios in ["(0.,0.,0.)", "(1.,0.)"] {
        let bad = input.replace("(1.,0.,0.)", ratios);
        let err = run(&bad).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Placement);
        assert!(err.source.is_some());
    }
    let bad = input.replace("#70,#80,$)", "#70,#80,#80)");
    assert_eq!(
        run(&bad).unwrap_err().math_error,
        Some(tessstep_math::Error::ParallelAxes)
    );
}
#[test]
fn product_cartesian_operators_reflections_nonuniform_and_mapping() {
    let operator = "#90=CARTESIAN_TRANSFORMATION_OPERATOR_3D_NON_UNIFORM('operator',$,#91,#70,2.,$,3.,4.);#91=DIRECTION('',(0.,-1.,0.));";
    let input = extra(&INPUT.replace("(#35,#34,#40)", "(#35,#34,#90)"), operator);
    let model = run(&input).unwrap();
    let t = model.parts().relationship_transforms[0].rep_1_to_rep_2;
    close(point(t, [1., 1., 1.]), [2.1, -3., 4.]);
    assert_eq!(model.parts().relationships[0].operator, Some(ItemId(90)));
    let mapped = extra(
        INPUT,
        &format!(
            "{operator}#92=REPRESENTATION_MAP(#32,#35);#93=MAPPED_ITEM('mapped',#92,#90);#94=REPRESENTATION('usage',(#93),#23);"
        ),
    );
    let m = run(&mapped).unwrap();
    let t = m.parts().mapped_transforms[0].source_to_using;
    close(point(t, [0.0254, 0., 0.]), [0.1, 0., 0.]);
    close(point(t, [1.0254, 1., 1.]), [2.1, -3., 4.]);
    let bad = input.replace("#70,2.,$,3.,4.", "#70,0.,$,3.,4.");
    assert_eq!(run(&bad).unwrap_err().kind, ErrorKind::Placement);
    let bad = input.replace("(0.,-1.,0.)", "(0.,0.,1.)");
    assert_eq!(run(&bad).unwrap_err().kind, ErrorKind::Placement);
}
#[test]
fn product_indirect_contexts_associations_and_uncertainties() {
    let input = INPUT
        .replace("(#30,#31),#23", "(#81),#23")
        .replace(
            "SHAPE_DEFINITION_REPRESENTATION(#9,#34)",
            "SHAPE_DEFINITION_REPRESENTATION(#9,#82)",
        )
        .replace(
            "#23=MODEL_CONTEXT(3,(#20,#21,#22));",
            "#23=UNCERTAIN_CONTEXT(3,(#20,#21,#22),(#84));",
        );
    let input = extra(
        &input,
        "#81=ITEM_GROUP('placements',(#30,#31));#82=REPRESENTATION('shape root',(#81),#23);#83=SHAPE_REPRESENTATION_RELATIONSHIP(#82,#34);#84=UNCERTAINTY_MEASURE_WITH_UNIT(0.01,#20,'accuracy','export tolerance');",
    );
    let model = run(&input).unwrap();
    assert!(
        model
            .items_in_context(ContextId(23))
            .unwrap()
            .contains(&ItemId(70))
    );
    assert!(
        model
            .representations_of(DefinitionId(5))
            .unwrap()
            .contains(&RepresentationId(34))
    );
    assert_eq!(model.parts().uncertainties.len(), 1);
    let u = &model.parts().uncertainties[0];
    assert_eq!(u.dimension, Dimension::Length);
    assert_eq!(u.value_si, 0.00001);
    assert_eq!(u.description.as_deref(), Some("export tolerance"));
    let instances = model
        .expand(
            DefinitionId(5),
            RepresentationId(82),
            &BTreeMap::new(),
            Default::default(),
        )
        .unwrap();
    close(
        point(instances[1].local_to_world, [0.0254, 0., 0.]),
        [0.1, 0., 0.],
    );
    assert_eq!(
        run(&input.replace("WITH_UNIT(0.01,", "WITH_UNIT(-0.01,"))
            .unwrap_err()
            .kind,
        ErrorKind::Units
    );
    // A cyclic reference between items terminates without recursive traversal.
    let cyclic = extra(
        &input.replace("(#30,#31));", "(#30,#31,#85));"),
        "#85=ITEM_GROUP('back',(#81));",
    );
    assert!(run(&cyclic).is_ok());
}
#[test]
fn product_nested_assembly_composition_choices_and_limits() {
    let input = extra(
        INPUT,
        "#100=PRODUCT('leaf','Leaf');#101=PRODUCT_DEFINITION_FORMATION('v1',#100);#102=PRODUCT_DEFINITION('design',#101);#103=NEXT_ASSEMBLY_USAGE_OCCURRENCE('nested',#6,#102);#104=PRODUCT_DEFINITION_SHAPE(#102);#105=PRODUCT_DEFINITION_SHAPE(#103);#106=REPRESENTATION('leaf',(#107),#23);#107=AXIS2_PLACEMENT_3D('origin',#108,$,$);#108=CARTESIAN_POINT('',(0.,0.,0.));#109=SHAPE_DEFINITION_REPRESENTATION(#104,#106);#110=ITEM_DEFINED_TRANSFORMATION(#107,#32);#111=REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#106,#35,#110);#112=CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#111,#105);",
    );
    let model = run(&input).unwrap();
    let instances = model
        .expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(instances.len(), 5);
    assert_eq!(instances[2].parent, Some(1));
    assert_eq!(instances[4].parent, Some(3));
    close(
        point(instances[2].local_to_world, [0., 0., 0.]),
        [0.1, 0., 0.],
    );
    close(
        point(instances[4].local_to_world, [0., 0., 0.]),
        [0., 0.2, 0.],
    );
    for limits in [
        ExpansionLimits {
            max_instances: 4,
            ..Default::default()
        },
        ExpansionLimits {
            max_depth: 2,
            ..Default::default()
        },
        ExpansionLimits {
            max_work: 1,
            ..Default::default()
        },
    ] {
        assert_eq!(
            model.expand(
                DefinitionId(5),
                RepresentationId(34),
                &BTreeMap::new(),
                limits
            ),
            Err(Error::ResourceLimit)
        );
    }
    assert_eq!(
        model
            .expand(
                DefinitionId(5),
                RepresentationId(34),
                &BTreeMap::new(),
                ExpansionLimits {
                    max_instances: 5,
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
        5
    );
    let rotated = extra(
        &input.replace("placement',#70,$,$)", "placement',#70,$,#121)"),
        "#121=DIRECTION('',(0.,1.,0.));",
    );
    let rotated = run(&rotated)
        .unwrap()
        .expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default(),
        )
        .unwrap();
    close(
        point(rotated[2].local_to_world, [1., 0., 0.]),
        [0.1, 1., 0.],
    );
    let ambiguous = extra(
        INPUT,
        "#120=CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#43,#11);",
    );
    let model = run(&ambiguous).unwrap();
    assert_eq!(
        model.expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default()
        ),
        Err(Error::AmbiguousPlacement)
    );
    assert!(
        model
            .expand(
                DefinitionId(5),
                RepresentationId(34),
                &BTreeMap::from([(OccurrenceId(7), RelationshipId(42))]),
                Default::default()
            )
            .is_ok()
    );
    let missing = INPUT.replace(
        "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#35,#34,#40)",
        "REPRESENTATION_RELATIONSHIP(#35,#34)",
    );
    assert_eq!(
        run(&missing).unwrap().expand(
            DefinitionId(5),
            RepresentationId(34),
            &BTreeMap::new(),
            Default::default()
        ),
        Err(Error::MissingPlacement)
    );
}
