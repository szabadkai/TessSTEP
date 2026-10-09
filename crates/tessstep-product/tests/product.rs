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
            solid_angle: Some(mm),
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

#[test]
fn product_expansion_is_iterative_bounded_and_does_not_invent_placements() {
    use std::collections::BTreeMap;
    let mut p = parts();
    p.definitions.clear();
    p.occurrences.clear();
    let length = Unit::new(Dimension::Length, 1.).unwrap();
    p.contexts.push(Context {
        id: ContextId(1),
        units: Units {
            length,
            plane_angle: Unit::new(Dimension::PlaneAngle, 1.).unwrap(),
            solid_angle: Some(Unit::new(Dimension::SolidAngle, 1.).unwrap()),
        },
    });
    for i in 0..2000_u64 {
        p.definitions.push(Definition {
            id: DefinitionId(i),
            formation: FormationId(2),
            identifier: String::new(),
        });
        p.representations.push(Representation {
            id: RepresentationId(i),
            name: String::new(),
            context: ContextId(1),
            items: vec![],
        });
        p.shapes.push(Shape {
            id: ShapeId(i),
            target: ShapeTarget::Definition(DefinitionId(i)),
        });
        p.shape_bindings.push(ShapeBinding {
            shape: ShapeId(i),
            representation: RepresentationId(i),
        });
        if i > 0 {
            p.occurrences.push(Occurrence {
                id: OccurrenceId(i),
                identifier: String::new(),
                parent: DefinitionId(i - 1),
                child: DefinitionId(i),
            });
            p.relationships.push(Relationship {
                id: RelationshipId(i),
                rep_1: RepresentationId(i),
                rep_2: RepresentationId(i - 1),
                transform: None,
                operator: Some(ItemId(i)),
            });
            p.placements.push(OccurrencePlacement {
                occurrence: OccurrenceId(i),
                relationship: RelationshipId(i),
            });
            p.relationship_transforms.push(ResolvedRelationship {
                relationship: RelationshipId(i),
                rep_1_to_rep_2: Transform::new(
                    [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                    [1., 0., 0.],
                )
                .unwrap(),
            });
        }
    }
    let model = Model::new(p.clone(), 1_000_000).unwrap();
    let limits = ExpansionLimits {
        max_depth: 2000,
        ..Default::default()
    };
    let instances = model
        .expand(
            DefinitionId(0),
            RepresentationId(0),
            &BTreeMap::new(),
            limits,
        )
        .unwrap();
    assert_eq!(instances.len(), 2000);
    assert_eq!(instances[1999].parent, Some(1998));
    assert_eq!(
        instances[1999].local_to_world.translation(),
        [1999., 0., 0.]
    );
    assert_eq!(
        model.expand(
            DefinitionId(0),
            RepresentationId(0),
            &BTreeMap::new(),
            ExpansionLimits {
                max_instances: 100,
                ..limits
            }
        ),
        Err(Error::ResourceLimit)
    );
    p.relationship_transforms.clear();
    assert_eq!(
        Model::new(p, 1_000_000).unwrap().expand(
            DefinitionId(0),
            RepresentationId(0),
            &BTreeMap::new(),
            limits
        ),
        Err(Error::MissingPlacement)
    );
}

#[test]
fn mapped_items_place_occurrences_without_relationships() {
    use std::collections::BTreeMap;
    let unit = |d| Unit::new(d, 1.).unwrap();
    let translate =
        |x| Transform::new([[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [x, 0., 0.]).unwrap();
    let rep = |id, items: &[u64]| Representation {
        id: RepresentationId(id),
        name: String::new(),
        context: ContextId(1),
        items: items.iter().map(|&i| ItemId(i)).collect(),
    };
    let mut p = parts();
    p.definitions.truncate(2);
    p.occurrences.truncate(1);
    p.contexts.push(Context {
        id: ContextId(1),
        units: Units {
            length: unit(Dimension::Length),
            plane_angle: unit(Dimension::PlaneAngle),
            solid_angle: None,
        },
    });
    // Parent 3 holds mapped item 100 of child 4's representation 11; occurrence 6
    // has its own representation 12 holding the same item.
    p.representations = vec![rep(10, &[100, 102]), rep(11, &[110]), rep(12, &[100])];
    for (shape, target, representation) in [
        (1, ShapeTarget::Definition(DefinitionId(3)), 10),
        (2, ShapeTarget::Definition(DefinitionId(4)), 11),
        (3, ShapeTarget::Occurrence(OccurrenceId(6)), 12),
    ] {
        p.shapes.push(Shape {
            id: ShapeId(shape),
            target,
        });
        p.shape_bindings.push(ShapeBinding {
            shape: ShapeId(shape),
            representation: RepresentationId(representation),
        });
    }
    p.maps.push(RepresentationMap {
        id: MapId(30),
        representation: RepresentationId(11),
        origin: ItemId(110),
    });
    for (item, target) in [(100, 101), (102, 103)] {
        p.mapped_items.push(MappedItem {
            id: ItemId(item),
            map: MapId(30),
            target: ItemId(target),
        });
        p.item_dependencies.push((ItemId(item), ItemId(target)));
    }
    for (item, x) in [(100, 5.), (102, 7.)] {
        p.mapped_transforms.push(ResolvedMapping {
            item: ItemId(item),
            using_representation: RepresentationId(10),
            source_to_using: translate(x),
        });
    }
    let expand = |p: &Parts| {
        Model::new(p.clone(), 100_000).unwrap().expand(
            DefinitionId(3),
            RepresentationId(10),
            &BTreeMap::new(),
            ExpansionLimits::default(),
        )
    };
    let instances = expand(&p).unwrap();
    assert_eq!(instances.len(), 2);
    assert_eq!(
        (instances[1].occurrence, instances[1].representation),
        (Some(OccurrenceId(6)), RepresentationId(11))
    );
    assert_eq!(instances[1].local_to_world.translation(), [5., 0., 0.]);
    // Two qualifying items in the occurrence's representation are ambiguous.
    let mut ambiguous = p.clone();
    ambiguous.representations[2].items.push(ItemId(102));
    assert_eq!(expand(&ambiguous), Err(Error::AmbiguousPlacement));
    // Without the occurrence's own representation, or when only that representation
    // (whose frame is not the parent's) holds the item, nothing places the occurrence.
    let mut unbound = p.clone();
    unbound.shape_bindings.pop();
    assert_eq!(expand(&unbound), Err(Error::MissingPlacement));
    let mut own_frame = p;
    own_frame.representations[0].items = vec![ItemId(102)];
    own_frame.mapped_transforms[0].using_representation = RepresentationId(12);
    assert_eq!(expand(&own_frame), Err(Error::MissingPlacement));
}
