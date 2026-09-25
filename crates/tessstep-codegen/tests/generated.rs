use std::{fs, path::PathBuf, process::Command};
use tessstep_codegen::{Error, Limits, generate};
use tessstep_express::{Source, compile};
fn compilation(text: &str) -> tessstep_express::Compilation {
    compile(
        &[Source {
            name: "test.exp",
            text,
        }],
        Default::default(),
    )
}
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn generated_bindings_compile_and_reflect_deterministically() {
    let sources = [
        Source {
            name: "base.exp",
            text: include_str!("../../../corpus/express/valid/base.exp"),
        },
        Source {
            name: "imports.exp",
            text: include_str!("../../../corpus/express/valid/imports.exp"),
        },
        Source {
            name: "opaque.exp",
            text: include_str!("../../../corpus/express/valid/opaque.exp"),
        },
        Source {
            name: "edge\"\\\n\té.exp",
            text: r#"SCHEMA early; USE FROM edge(root); ENTITY sub SUBTYPE OF(root); END_ENTITY; END_SCHEMA;
SCHEMA edge;
TYPE bool = BOOLEAN; END_TYPE;
TYPE numeric_value = NUMBER; END_TYPE;
TYPE int = INTEGER; END_TYPE;
TYPE bits = BINARY(7) FIXED; END_TYPE;
TYPE logic = LOGICAL; END_TYPE;
TYPE aliased = int; END_TYPE;
ENTITY root; match : STRING; state:OPTIONAL ENUMERATION OF(on, off); END_ENTITY;
ENTITY left SUBTYPE OF(root); next:OPTIONAL right; END_ENTITY;
ENTITY right SUBTYPE OF(root); back:OPTIONAL left; END_ENTITY;
ENTITY diamond SUBTYPE OF(left,right); vals:LIST OF LIST OF INTEGER;
inline_enum: OPTIONAL ENUMERATION OF(rust_self, rust_self_, match);
inline_select: OPTIONAL SELECT(left,int);
END_ENTITY;
END_SCHEMA;"#,
        },
    ];
    let c = compile(&sources, Default::default());
    assert!(c.ir.is_some(), "{:?}", c.diagnostics);
    let source = generate(&c, Limits::default()).unwrap();
    assert_eq!(
        source,
        generate(&compile(&sources, Default::default()), Limits::default()).unwrap()
    );
    // Strict exact-size budget includes UTF-8 escaped source strings and final newline.
    assert_eq!(
        source,
        generate(
            &c,
            Limits {
                max_output_bytes: source.len(),
                ..Limits::default()
            }
        )
        .unwrap()
    );
    assert_eq!(
        generate(
            &c,
            Limits {
                max_output_bytes: source.len() - 1,
                ..Limits::default()
            }
        ),
        Err(Error::OutputLimit)
    );
    let temp = Temp(std::env::temp_dir().join(format!("tessstep-codegen-{}", std::process::id())));
    fs::create_dir_all(&temp.0).unwrap();
    let schema = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tessstep-schema/src/lib.rs");
    let library = temp.0.join("libtessstep_schema.rlib");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    run(Command::new(&rustc)
        .arg("--edition=2024")
        .arg("--crate-type=rlib")
        .arg("--crate-name=tessstep_schema")
        .arg(&schema)
        .arg("-o")
        .arg(&library));
    fs::write(temp.0.join("bindings.rs"), source).unwrap();
    let consumer = r#"
#![forbid(unsafe_code)]
mod generated { include!("bindings.rs"); }
use generated::*;
use tessstep_schema::{AttributeKind, DeclarationKind, Domain, EntityBinding, EntityRef, Logical};
fn main() {
    let part = schema_base::Entity_PART {
        attr_name: "part".into(),
        attr_samples: vec![Some(schema_base::Type_MEASURE(1.0)),None,Some(schema_base::Type_MEASURE(3.0))],
        attr_tint: None,
    };
    assert_eq!(part.attr_name,"part");
    let reference = EntityRef::<schema_consumer::Entity_COMPONENT>::new(7.try_into().unwrap());
    let _: EntityRef<schema_base::Entity_PART> = reference;
    let item: EntityRef<schema_base::Entity_ITEM> = reference.upcast();
    assert_eq!(item.id().get(),7);
    // Recursive entity graphs are references, not infinitely sized values.
    let _left = schema_edge::Entity_LEFT { attr_match: "root".into(), attr_state: None, attr_next: None };
    let root = schema_edge::Entity_ROOT { attr_match: "root".into(), attr_state: _left.attr_state };
    let _early = schema_early::Entity_SUB { attr_match: "root".into(), attr_state: root.attr_state };
    let _diamond = schema_edge::Entity_DIAMOND { attr_match: "root".into(), attr_state: None, attr_next: None, attr_back: None, attr_vals: vec![], attr_inline_enum: None, attr_inline_select: None };
    let _tone = schema_base::Type_TONE(schema_base::Domain0::Member_WARM);
    let _choice = schema_base::Type_CHOICE(schema_base::Domain1::Alternative_3(reference));
    let _right = schema_edge::Entity_RIGHT { attr_match: "root".into(), attr_state: None, attr_back: None };
    assert_eq!(schema_edge::Type_LOGIC(Logical::Unknown).0, Logical::Unknown);
    let schema = SCHEMA_SET.schema("consumer").unwrap();
    let component = schema.lookup("component").unwrap();
    assert_eq!(component,schema_base::Entity_PART::DECLARATION);
    let base = SCHEMA_SET.schema("BASE").unwrap();
    assert_eq!(component,base.lookup("PART").unwrap());
    assert!(schema.exports.iter().all(|s|s.name != "FACTOR"));
    assert!(schema.lookup("FACTOR").is_some());
    let d = SCHEMA_SET.declaration(component).unwrap();
    assert_eq!(d.span.source, 0);
    assert!(d.span.start < d.span.end && d.span.line > 0 && d.span.column > 0);
    let DeclarationKind::Entity { supertypes, attributes, unique, .. } = d.kind else { panic!() };
    assert_eq!(supertypes,&[base.lookup("ITEM").unwrap()]);
    assert_eq!(attributes.len(),3); // local metadata; inherited explicit field is in record
    let Domain::Aggregate { bounds: Some((lo,hi)), optional, unique: element_unique, element, .. } = attributes[0].domain else { panic!() };
    assert_eq!((lo.text,hi.text),("1","3")); assert!(optional && element_unique);
    assert!(matches!(element,Domain::Named(_)));
    assert!(attributes[1].optional);
    assert!(matches!(attributes[2].kind,AttributeKind::Derived(_)));
    assert_eq!(unique[0].expression.text,"name");
    let DeclarationKind::Entity { abstract_entity, supertype_constraint, attributes, where_rules, .. } = SCHEMA_SET.declaration(base.lookup("ITEM").unwrap()).unwrap().kind else { panic!() };
    assert!(abstract_entity); assert!(supertype_constraint.is_some());
    assert_eq!(where_rules[0].expression.text,"LENGTH(name) > 0");
    assert!(matches!(attributes[1].kind,AttributeKind::Inverse { attribute: "TARGET", .. }));
    let DeclarationKind::Type { domain: Domain::Select(choices), .. } = SCHEMA_SET.declaration(base.lookup("CHOICE").unwrap()).unwrap().kind else { panic!() };
    assert_eq!(choices,&[component,base.lookup("MEASURE").unwrap()]);
    let DeclarationKind::Type { domain: Domain::Enumeration(names), .. } = SCHEMA_SET.declaration(base.lookup("TONE").unwrap()).unwrap().kind else { panic!() };
    assert_eq!(names,&["WARM","COOL"]);
    let DeclarationKind::Constant { value, .. } = SCHEMA_SET.declaration(base.lookup("FACTOR").unwrap()).unwrap().kind else { panic!() };
    assert_eq!(value.text,"2.5");
    let algorithms = SCHEMA_SET.schema("algorithms").unwrap();
    assert!(matches!(SCHEMA_SET.declaration(algorithms.lookup("TWICE").unwrap()).unwrap().kind,DeclarationKind::Unsupported { .. }));
    assert!(!SCHEMA_SET.unsupported.is_empty());
    assert_eq!(SCHEMA_SET.sources[0],"base.exp");
    assert_eq!(SCHEMA_SET.sources[3], "edge\"\\\n\té.exp");
    assert!(SCHEMA_SET.declaration(tessstep_schema::DeclarationId(usize::MAX)).is_none());
}
"#;
    fs::write(temp.0.join("main.rs"), consumer).unwrap();
    let executable = temp
        .0
        .join(format!("consumer{}", std::env::consts::EXE_SUFFIX));
    run(Command::new(&rustc)
        .arg("--edition=2024")
        .arg("--deny=warnings")
        .arg("--allow=dead_code")
        .arg(temp.0.join("main.rs"))
        .arg("--extern")
        .arg(format!("tessstep_schema={}", library.display()))
        .arg("-o")
        .arg(&executable));
    run(&mut Command::new(&executable));
    // Nominal references prevent accidental assignment across unrelated entities.
    fs::write(temp.0.join("bad.rs"), "mod generated { include!(\"bindings.rs\"); } fn main() { let x = tessstep_schema::EntityRef::<generated::schema_base::Entity_ITEM>::new(1.try_into().unwrap()); let _: tessstep_schema::EntityRef<generated::schema_base::Entity_PART> = x; }").unwrap();
    let bad = Command::new(&rustc)
        .arg("--edition=2024")
        .arg(temp.0.join("bad.rs"))
        .arg("--extern")
        .arg(format!("tessstep_schema={}", library.display()))
        .arg("-o")
        .arg(temp.0.join("bad"))
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("mismatched types"));
}
#[test]
fn generation_limits_and_invalid_compilation() {
    let mut c = compilation("SCHEMA a; ENTITY node; child:OPTIONAL node; END_ENTITY; END_SCHEMA;");
    assert_eq!(
        generate(
            &c,
            Limits {
                max_work: 0,
                ..Limits::default()
            }
        ),
        Err(Error::WorkLimit)
    );
    assert_eq!(
        generate(
            &c,
            Limits {
                max_output_bytes: 0,
                ..Limits::default()
            }
        ),
        Err(Error::OutputLimit)
    );
    assert_eq!(
        generate(&compilation("broken"), Limits::default()),
        Err(Error::InvalidCompilation)
    );
    c.ir.as_mut().unwrap().declarations[0].schema = usize::MAX;
    assert_eq!(
        generate(&c, Limits::default()),
        Err(Error::InvalidCompilation)
    );
}
#[path = "../../../fuzz/support/codegen.rs"]
mod fuzz_support;
#[test]
fn codegen_fuzz_smoke() {
    let seeds: &[&[u8]] = &[
        include_bytes!("../../../corpus/express/valid/base.exp"),
        include_bytes!("../../../corpus/express/valid/opaque.exp"),
        b"SCHEMA a; TYPE val=SELECT(n); END_TYPE; ENTITY n; values:LIST OF val; END_ENTITY; END_SCHEMA;",
    ];
    let mut cases = 0;
    for seed in seeds {
        for end in 0..=seed.len() {
            fuzz_support::exercise(&seed[..end]);
            cases += 1;
        }
        for i in 0..seed.len() {
            for byte in [0, b' ', b'(', b'\\', b'\"', b';', 255] {
                let mut mutated = seed.to_vec();
                mutated[i] = byte;
                fuzz_support::exercise(&mutated);
                cases += 1;
            }
        }
    }
    let mut rng = 41u64;
    for len in 0..512 {
        let bytes: Vec<_> = (0..len)
            .map(|_| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                (rng & 127) as u8
            })
            .collect();
        fuzz_support::exercise(&bytes);
        cases += 1;
    }
    println!("codegen fuzz smoke: {cases} bounded cases");
}

#[test]
fn generated_metadata_decodes_physical_instances() {
    let c = compilation(include_str!("../../../corpus/schema/sample.exp"));
    let generated = generate(&c, Limits::default()).unwrap();
    let temp =
        Temp(std::env::temp_dir().join(format!("tessstep-decode-consumer-{}", std::process::id())));
    fs::create_dir_all(&temp.0).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    for name in ["schema", "part21", "model"] {
        let mut cmd = Command::new(&rustc);
        cmd.arg("--edition=2024")
            .arg("--crate-type=rlib")
            .arg(format!("--crate-name=tessstep_{name}"))
            .arg(root.join(format!("tessstep-{name}/src/lib.rs")))
            .arg("-L")
            .arg(&temp.0)
            .arg("-o")
            .arg(temp.0.join(format!("libtessstep_{name}.rlib")));
        if name == "model" {
            for dep in ["schema", "part21"] {
                cmd.arg("--extern").arg(format!(
                    "tessstep_{dep}={}",
                    temp.0.join(format!("libtessstep_{dep}.rlib")).display()
                ));
            }
        }
        run(&mut cmd);
    }
    fs::write(temp.0.join("bindings.rs"), generated).unwrap();
    fs::write(temp.0.join("main.rs"), r##"
mod generated { include!("bindings.rs"); }
fn main() {
    use tessstep_model::decode::{decode, Limits, ErrorKind};
    let input = "ISO-10303-21;HEADER;FILE_DESCRIPTION(('test'),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('SAMPLE'));ENDSEC;DATA;#1=NODE(2,#2);#2=CHILD(3,#1,'child');ENDSEC;END-ISO-10303-21;";
    let doc = tessstep_model::parse(input.as_bytes(), Default::default()).unwrap();
    let decoded = decode(&doc, &generated::SCHEMA_SET, "sample", Limits::default()).unwrap();
    assert_eq!(decoded.entities().len(), 2);
    assert_eq!(decoded.entities()[1].attributes[2].declaration.name, "LABEL");
    let bad = input.replace("CHILD(3", "CHILD('bad'");
    let doc = tessstep_model::parse(bad.as_bytes(), Default::default()).unwrap();
    assert_eq!(decode(&doc, &generated::SCHEMA_SET, "sample", Limits::default()).unwrap_err().kind, ErrorKind::TypeMismatch);
    let extended = input.replace("ENDSEC;END-ISO", "#3=(CHILD('label')NODE(4,#2)OTHER(.T.));#4=HOLDER(#3,(1,2),'ok');#5=HOLDER(COUNT(7),(3),'hi');#6=HOLDER(COLOR(.RED.),(4,5),'ab');ENDSEC;END-ISO");
    let doc = tessstep_model::parse(extended.as_bytes(), Default::default()).unwrap();
    let decoded = decode(&doc, &generated::SCHEMA_SET, "sample", Limits::default()).unwrap();
    assert_eq!(decoded.entities().len(), 6);
    assert_eq!(decoded.entities()[2].declaration, None);
    let bad = extended.replace("(1,2),'ok'", "(1,1),'ok'");
    let doc = tessstep_model::parse(bad.as_bytes(), Default::default()).unwrap();
    assert_eq!(decode(&doc, &generated::SCHEMA_SET, "sample", Limits::default()).unwrap_err().kind, ErrorKind::DuplicateValue);
}
"##).unwrap();
    let binary = temp
        .0
        .join(format!("consumer{}", std::env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(&rustc);
    cmd.arg("--edition=2024")
        .arg(temp.0.join("main.rs"))
        .arg("-L")
        .arg(&temp.0)
        .arg("-o")
        .arg(&binary);
    for name in ["schema", "model"] {
        cmd.arg("--extern").arg(format!(
            "tessstep_{name}={}",
            temp.0.join(format!("libtessstep_{name}.rlib")).display()
        ));
    }
    run(&mut cmd);
    run(&mut Command::new(&binary));
    let validator_source = tessstep_codegen::generate_validator(&c, Limits::default()).unwrap();
    assert_eq!(
        validator_source,
        tessstep_codegen::generate_validator(
            &c,
            Limits {
                max_output_bytes: validator_source.len(),
                ..Limits::default()
            }
        )
        .unwrap()
    );
    assert_eq!(
        tessstep_codegen::generate_validator(
            &c,
            Limits {
                max_output_bytes: validator_source.len() - 1,
                ..Limits::default()
            }
        ),
        Err(Error::OutputLimit)
    );
    fs::write(temp.0.join("main.rs"), validator_source).unwrap();
    run(&mut cmd);
    let fixture = temp.0.join("input.step");
    let valid = include_str!("../../../corpus/part21/valid/empty.step")
        .replace("DATA;", "DATA;#1=HOLDER(COUNT(7),(1,2),'ok');");
    for (schema, text, status, exit) in [
        ("sample", valid.clone(), "accepted", 0),
        ("sample", valid.replace("(1,2)", "(1,1)"), "rejected", 1),
        ("missing", valid, "not_configured", 1),
        ("sample", "not STEP".into(), "not_run", 1),
    ] {
        fs::write(&fixture, text).unwrap();
        let output = Command::new(&binary)
            .arg(schema)
            .arg(&fixture)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(exit));
        let json = String::from_utf8(output.stdout).unwrap();
        assert!(json.contains(&format!("\"status\":\"{status}\"")), "{json}");
    }
}

#[test]
fn decoder_fuzz_metadata_matches_authored_schema() {
    let c = compile(
        &[Source {
            name: "corpus/schema/sample.exp",
            text: include_str!("../../../corpus/schema/sample.exp"),
        }],
        Default::default(),
    );
    assert_eq!(
        generate(&c, Limits::default()).unwrap(),
        include_str!("../../../fuzz/support/decoder_schema.rs")
    );
}
