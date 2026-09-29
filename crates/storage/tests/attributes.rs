use siner_storage::{AttributeValue, Attributes};

#[test]
fn attributes_keep_the_json_of_the_day_files() {
    let stored = r#"{"a.bool":true,"a.double":2.0,"a.int":7,"a.list":[1,"x",null],"a.map":{"k":1.5},"a.null":null,"a.text":"hi"}"#;
    let attributes = Attributes::from_json(stored).unwrap();
    assert_eq!(attributes.to_json(), stored);
    assert_eq!(attributes["a.int"], AttributeValue::Int(7));
    assert_eq!(attributes["a.double"], AttributeValue::Double(2.0));
    assert_eq!(attributes["a.text"], "hi");
    assert_eq!(attributes["missing"], AttributeValue::Null);
    assert_eq!(
        attributes["a.list"],
        AttributeValue::Array(vec![
            AttributeValue::Int(1),
            AttributeValue::String("x".into()),
            AttributeValue::Null,
        ])
    );
}

#[test]
fn a_value_prints_as_json_and_names_its_type() {
    let names: Vec<_> = [
        AttributeValue::Null,
        true.into(),
        7_i64.into(),
        1.5.into(),
        "x".into(),
        AttributeValue::Array(Vec::new()),
        AttributeValue::Map(Attributes::new()),
    ]
    .iter()
    .map(|value| format!("{value} {}", value.type_name()))
    .collect();
    assert_eq!(
        names,
        [
            "null null",
            "true bool",
            "7 int",
            "1.5 float",
            "\"x\" string",
            "[] array",
            "{} object"
        ]
    );
}
