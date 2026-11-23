use super::prune::prune_unreferenced;
use handl_plugin_codegen::{Arg, Function, Record, SdkgenModule, Typedef};

fn record(name: &str) -> Record {
    Record {
        name: name.into(),
        fields: vec![],
        size: 0,
        align: 0,
        doc: None,
    }
}

fn typedef(name: &str) -> Typedef {
    Typedef {
        name: name.into(),
        port: name.into(),
        descriptor: "i32".into(),
        doc: None,
    }
}

fn function(out_struct: Option<&str>, arg_port: &str, ret: &str) -> Function {
    Function {
        name: "f".into(),
        port_name: "F".into(),
        port_args: vec![Arg {
            name: "a".into(),
            port: arg_port.into(),
        }],
        arg_descriptors: vec!["i32".into()],
        return_port: ret.into(),
        ret_descriptor: "i32".into(),
        out_struct: out_struct.map(|s| s.into()),
        variadic: false,
        errno_guard: true,
        c_decl: String::new(),
        doc: None,
    }
}

#[test]
fn keeps_only_referenced_types() {
    let mut m = SdkgenModule {
        records: vec![record("used_out"), record("unused")],
        typedefs: vec![typedef("used_td"), typedef("unused_td")],
        functions: vec![function(Some("used_out"), "used_td", "Int")],
        ..Default::default()
    };
    prune_unreferenced(&mut m);
    assert_eq!(m.records.len(), 1);
    assert_eq!(m.records[0].name, "used_out");
    assert_eq!(m.typedefs.len(), 1);
    assert_eq!(m.typedefs[0].name, "used_td");
}

#[test]
fn return_port_also_keeps_record() {
    let mut m = SdkgenModule {
        records: vec![record("ret_rec"), record("unused")],
        functions: vec![function(None, "Int", "ret_rec")],
        ..Default::default()
    };
    prune_unreferenced(&mut m);
    assert_eq!(m.records.len(), 1);
    assert_eq!(m.records[0].name, "ret_rec");
}

#[test]
fn no_functions_is_a_noop() {
    let mut m = SdkgenModule {
        records: vec![record("a"), record("b")],
        typedefs: vec![typedef("t")],
        ..Default::default()
    };
    prune_unreferenced(&mut m);
    assert_eq!(m.records.len(), 2);
    assert_eq!(m.typedefs.len(), 1);
}
