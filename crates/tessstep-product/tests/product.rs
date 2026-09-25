use tessstep_product::*;

fn parts() -> Parts {
    Parts {
        products: vec![Product {
            id: ProductId(u64::MAX),
            identifier: "part".into(),
            name: "Part".into(),
        }],
        formations: vec![Formation {
            id: FormationId(2),
            product: ProductId(u64::MAX),
            identifier: "v1".into(),
        }],
        definitions: (3..6)
            .map(|i| Definition {
                id: DefinitionId(i),
                formation: FormationId(2),
                identifier: "design".into(),
            })
            .collect(),
        occurrences: vec![
            Occurrence {
                id: OccurrenceId(6),
                identifier: "a".into(),
                parent: DefinitionId(3),
                child: DefinitionId(4),
            },
            Occurrence {
                id: OccurrenceId(7),
                identifier: "b".into(),
                parent: DefinitionId(3),
                child: DefinitionId(4),
            },
            Occurrence {
                id: OccurrenceId(8),
                identifier: "c".into(),
                parent: DefinitionId(4),
                child: DefinitionId(5),
            },
        ],
        ..Parts::default()
    }
}
#[test]
fn independent_product_graph_preserves_reuse_and_rejects_cycles() {
    let input = parts();
    let model = Model::new(input.clone(), 1000).unwrap();
    assert_eq!(model.roots(), &[DefinitionId(3)]);
    assert_eq!(model.parts().occurrences.len(), 3);
    assert_eq!(Model::new(input.clone(), 0), Err(Error::ResourceLimit));
    let mut cycle = input.clone();
    cycle.occurrences[2].child = DefinitionId(3);
    assert_eq!(Model::new(cycle, 1000), Err(Error::Cycle));
    let mut missing = input.clone();
    missing.definitions[0].formation = FormationId(999);
    assert_eq!(Model::new(missing, 1000), Err(Error::MissingLink));
    let mut duplicate = input;
    duplicate.occurrences[1].id = OccurrenceId(6);
    assert_eq!(Model::new(duplicate, 1000), Err(Error::DuplicateId));
}
#[test]
fn explicit_units_check_numeric_range_and_dimensions() {
    let mm = Unit::new(Dimension::Length, 0.001).unwrap();
    assert_eq!(mm.to_si(2500.).unwrap(), 2.5);
    assert!(mm.to_si(-0.).unwrap().is_sign_negative());
    assert_eq!(mm.to_si(f64::from_bits(1)), Err(Error::NumericRange));
    assert_eq!(mm.to_si(f64::INFINITY), Err(Error::NumericRange));
    assert_eq!(
        Unit::new(Dimension::Length, 1e20).unwrap().to_si(f64::MAX),
        Err(Error::NumericRange)
    );
    for scale in [0., -1., f64::INFINITY, f64::NAN] {
        assert_eq!(Unit::new(Dimension::Length, scale), Err(Error::InvalidUnit));
    }
    let mut input = parts();
    input.contexts.push(Context {
        id: ContextId(1),
        units: Units {
            length: mm,
            plane_angle: mm,
            solid_angle: mm,
        },
    });
    assert_eq!(Model::new(input, 1000), Err(Error::InvalidUnit));
}
#[test]
fn product_graph_is_iterative_and_budgeted() {
    let mut input = parts();
    input.definitions.clear();
    input.occurrences.clear();
    for i in 0..10_000 {
        input.definitions.push(Definition {
            id: DefinitionId(i),
            formation: FormationId(2),
            identifier: String::new(),
        });
        if i > 0 {
            input.occurrences.push(Occurrence {
                id: OccurrenceId(i),
                identifier: String::new(),
                parent: DefinitionId(i - 1),
                child: DefinitionId(i),
            });
        }
    }
    assert_eq!(Model::new(input.clone(), 100), Err(Error::ResourceLimit));
    assert_eq!(
        Model::new(input, 1_000_000).unwrap().roots(),
        &[DefinitionId(0)]
    );
}
