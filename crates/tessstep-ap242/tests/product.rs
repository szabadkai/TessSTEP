use tessstep_ap242::{ErrorKind, Limits, adapt};
use tessstep_model::{decode, parse};
use tessstep_part21::ParseLimits;
use tessstep_product::*;
#[allow(dead_code)]
mod schema {
    include!("support/schema.rs");
}
const ASSEMBLY: &str = include_str!("../../../corpus/product/assembly.step");
const MAPPED: &str = include_str!("../../../corpus/product/mapped.step");
fn run(input: &str, limits: Limits) -> Result<Model, tessstep_ap242::Error> {
    let doc = parse(input.as_bytes(), ParseLimits::default()).unwrap();
    let decoded = decode::decode(
        &doc,
        &schema::SCHEMA_SET,
        "product_test",
        decode::Limits::default(),
    )
    .unwrap();
    adapt(&decoded, "product_test", limits)
}
fn error(input: &str) -> tessstep_ap242::Error {
    run(input, Limits::default()).unwrap_err()
}

#[test]
fn product_adapter_follows_definitions_representations_and_occurrences() {
    let m = run(ASSEMBLY, Limits::default()).unwrap();
    let p = m.parts();
    assert_eq!(m.roots(), &[DefinitionId(5)]);
    assert_eq!(
        (p.products.len(), p.formations.len(), p.definitions.len()),
        (2, 2, 2)
    );
    assert_eq!(
        p.occurrences.iter().map(|o| o.child).collect::<Vec<_>>(),
        [DefinitionId(6), DefinitionId(6)]
    );
    assert_eq!(p.representations.len(), 2);
    assert_eq!(p.representations[1].items, [ItemId(32), ItemId(33)]);
    assert_eq!(p.shape_bindings[1].representation, RepresentationId(35));
    assert_eq!(p.contexts[0].units.length.scale(), 0.001);
    assert!((p.contexts[1].units.length.to_si(10.).unwrap() - 0.254).abs() < 1e-15);
    assert_eq!(p.relationships[0].rep_1, RepresentationId(35));
    assert_eq!(
        p.relationships[0].transform,
        Some(ItemTransform {
            item_1: ItemId(32),
            item_2: ItemId(30)
        })
    );
    assert_eq!(p.placements[1].occurrence, OccurrenceId(8));
    assert_eq!(p.placements[1].relationship, RelationshipId(43));
    assert_eq!(run(ASSEMBLY, Limits::default()).unwrap(), m);
    // Owned model is usable after physical and decoded documents are gone.
    assert_eq!(p.products[1].name, "Reusable part");
}
#[test]
fn product_adapter_complex_membership_and_mapping_reuse() {
    let complex = run(
        include_str!("../../../corpus/product/complex.step"),
        Limits::default(),
    )
    .unwrap();
    assert_eq!(complex, run(ASSEMBLY, Limits::default()).unwrap());
    let mapped = run(MAPPED, Limits::default()).unwrap();
    assert_eq!(
        mapped.parts().maps[0],
        RepresentationMap {
            id: MapId(50),
            representation: RepresentationId(35),
            origin: ItemId(32)
        }
    );
    assert_eq!(
        mapped.parts().mapped_items[0],
        MappedItem {
            id: ItemId(51),
            map: MapId(50),
            target: ItemId(30)
        }
    );
    assert_eq!(mapped.parts().representations.len(), 3);
    // Invalid direct origin/target membership is not silently repaired.
    assert_eq!(
        error(&MAPPED.replace("REPRESENTATION_MAP(#32,#35)", "REPRESENTATION_MAP(#30,#35)"))
            .graph_error,
        Some(Error::MissingLink)
    );
    let cycle = MAPPED.replace("REPRESENTATION_MAP(#32,#35)", "REPRESENTATION_MAP(#30,#52)");
    assert_eq!(error(&cycle).graph_error, Some(Error::Cycle));
}
#[test]
fn product_adapter_rejects_cycles_and_misbound_placements() {
    assert_eq!(
        error(include_str!("../../../corpus/product/cycle.step")).graph_error,
        Some(Error::Cycle)
    );
    let bad = ASSEMBLY.replace(
        "ITEM_DEFINED_TRANSFORMATION(#32,#30)",
        "ITEM_DEFINED_TRANSFORMATION(#30,#32)",
    );
    assert_eq!(error(&bad).graph_error, Some(Error::MissingLink));
    let bad = ASSEMBLY.replace(
        "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#35,#34,#40)",
        "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#35,#35,#40)",
    );
    assert_eq!(error(&bad).graph_error, Some(Error::MissingLink));
    let unsupported = ASSEMBLY.replace(
        "NEXT_ASSEMBLY_USAGE_OCCURRENCE('left'",
        "PRODUCT_DEFINITION_RELATIONSHIP('left'",
    );
    assert_eq!(error(&unsupported).kind, ErrorKind::Unsupported);
}
#[test]
fn product_adapter_unit_failures_are_explicit_and_located() {
    let missing = error(include_str!("../../../corpus/product/missing-units.step"));
    assert_eq!(missing.kind, ErrorKind::Units);
    assert_eq!(missing.entity.unwrap().get(), 23);
    assert!(missing.source.is_some());
    assert_eq!(
        error(include_str!(
            "../../../corpus/product/unsupported-context.step"
        ))
        .kind,
        ErrorKind::Unsupported
    );
    for (from, to) in [
        ("(3,(#20,#21,#22))", "(3,(#20,#26,#21,#22))"),
        ("MEASURE_WITH_UNIT(25.4,#20)", "MEASURE_WITH_UNIT(-1.,#20)"),
        ("MEASURE_WITH_UNIT(25.4,#20)", "MEASURE_WITH_UNIT(25.4,#26)"),
        ("MEASURE_WITH_UNIT(25.4,#20)", "MEASURE_WITH_UNIT(25.4,#21)"),
        ("DIMENSIONAL_EXPONENTS(1.,0.", "DIMENSIONAL_EXPONENTS(0.,0."),
        ("SI_LENGTH(.MILLI.,.METRE.)", "SI_LENGTH(.MILLI.,.RADIAN.)"),
    ] {
        assert_eq!(
            error(&ASSEMBLY.replace(from, to)).kind,
            ErrorKind::Units,
            "{to}"
        );
    }
    let offset = ASSEMBLY.replace(
        "CONVERTED_LENGTH(#24,'inch',#25)",
        "OFFSET_LENGTH(#24,'inch',#25,2.)",
    );
    assert_eq!(error(&offset).kind, ErrorKind::Unsupported);
}
#[test]
fn product_adapter_angle_conversion_and_resource_limits() {
    let angle = ASSEMBLY.replace("#21=SI_PLANE_ANGLE($,.RADIAN.);", "#21=CONVERTED_ANGLE(#61,'degree',#62);#61=DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.);#62=MEASURE_WITH_UNIT(0.017453292519943295,#63);#63=SI_PLANE_ANGLE($,.RADIAN.);");
    let model = run(&angle, Limits::default()).unwrap();
    assert!(
        (model.parts().contexts[0]
            .units
            .plane_angle
            .to_si(180.)
            .unwrap()
            - std::f64::consts::PI)
            .abs()
            < 1e-15
    );
    for limits in [
        Limits {
            max_work: 0,
            ..Limits::default()
        },
        Limits {
            max_text_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_unit_depth: 1,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            run(ASSEMBLY, limits).unwrap_err().kind,
            ErrorKind::ResourceLimit
        );
    }
}

#[test]
fn product_adapter_numeric_extremes_and_absent_transform() {
    for (prefix, factor) in [("EXA", "1.E300"), ("ATTO", "1.E-320")] {
        let input = ASSEMBLY.replace(".MILLI.", &format!(".{prefix}.")).replace(
            "MEASURE_WITH_UNIT(25.4,#20)",
            &format!("MEASURE_WITH_UNIT({factor},#20)"),
        );
        assert_eq!(error(&input).kind, ErrorKind::Units);
    }
    let input = ASSEMBLY.replace(
        "REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#35,#34,#40)",
        "REPRESENTATION_RELATIONSHIP(#35,#34)",
    );
    let model = run(&input, Limits::default()).unwrap();
    assert_eq!(model.parts().relationships[0].transform, None);
    assert_eq!(model.parts().placements.len(), 2);
}
#[test]
fn product_adapter_is_independent_of_record_order_and_metadata_roles() {
    let start = ASSEMBLY.find("DATA;").unwrap() + 5;
    let end = ASSEMBLY[start..].find("ENDSEC;").unwrap() + start;
    let mut rows = ASSEMBLY[start..end].lines().collect::<Vec<_>>();
    rows.reverse();
    let shuffled = format!(
        "{}{}\n{}",
        &ASSEMBLY[..start],
        rows.join("\n"),
        &ASSEMBLY[end..]
    );
    let m = run(&shuffled, Limits::default()).unwrap();
    assert_eq!(m.parts().occurrences.len(), 2);
    assert_eq!(m.roots(), &[DefinitionId(5)]);
    let doc = parse(ASSEMBLY.as_bytes(), ParseLimits::default()).unwrap();
    let d = decode::decode(
        &doc,
        &schema::SCHEMA_SET,
        "product_test",
        decode::Limits::default(),
    )
    .unwrap();
    assert_eq!(
        adapt(&d, "missing", Limits::default()).unwrap_err().kind,
        ErrorKind::Schema
    );
}
#[test]
fn product_adapter_fuzz_smoke() {
    let limits = ParseLimits {
        max_input_bytes: 16_384,
        max_entities: 256,
        ..ParseLimits::default()
    };
    let mut seed = 0x7234_u64;
    for i in 0..1500 {
        let mut bytes = MAPPED.as_bytes().to_vec();
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let pos = seed as usize % bytes.len();
        if i % 3 == 0 {
            bytes.truncate(pos);
        } else {
            bytes[pos] = (seed >> 24) as u8;
        }
        if let Ok(doc) = parse(bytes.as_slice(), limits) {
            if let Ok(decoded) = decode::decode(
                &doc,
                &schema::SCHEMA_SET,
                "product_test",
                decode::Limits {
                    max_work: 100_000,
                    max_depth: 32,
                },
            ) {
                let limits = Limits {
                    max_work: 50_000,
                    max_text_bytes: 4096,
                    max_unit_depth: 16,
                };
                assert_eq!(
                    adapt(&decoded, "product_test", limits),
                    adapt(&decoded, "product_test", limits)
                );
            }
        }
    }
}
