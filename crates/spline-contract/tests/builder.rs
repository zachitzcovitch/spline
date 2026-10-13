use spline_contract::{ContractBuilder, Error, OperationKind};
use spline_schema::{Context, Schema, Shape, TypeInfo};

struct Input;
impl Schema for Input {
    fn info() -> TypeInfo {
        TypeInfo::named("test.Input")
    }
    fn describe(_: &mut Context<'_>) -> spline_schema::Result<Shape> {
        Ok(Shape::String)
    }
}

#[test]
fn operation_registration_is_atomic_and_recoverable() {
    let mut builder = ContractBuilder::new("users", "").unwrap();
    assert!(matches!(
        builder.operation::<Input, u64, String>("bad", "bad", OperationKind::Query, ""),
        Err(Error::Schema(_))
    ));
    builder
        .operation::<String, String, String>("good", "good", OperationKind::Query, "")
        .unwrap();
    let contract = builder.finish().unwrap();
    assert_eq!(contract.operations().len(), 1);
    assert!(contract.schemas().definitions().is_empty());
}

#[test]
fn identifiers_duplicates_and_empty_contracts_are_validated() {
    for namespace in ["", ".users", "users.", "bad-name", "x..y"] {
        assert!(matches!(
            ContractBuilder::new(namespace, ""),
            Err(Error::InvalidIdentifier(_))
        ));
    }
    assert!(matches!(
        ContractBuilder::new("users", "").unwrap().finish(),
        Err(Error::EmptyContract)
    ));
    let mut builder = ContractBuilder::new("users", "").unwrap();
    assert!(matches!(
        builder.operation::<(), (), ()>("bad.name", "name", OperationKind::Query, ""),
        Err(Error::InvalidIdentifier(_))
    ));
    builder
        .operation::<(), (), ()>("get", "fetch", OperationKind::Query, "")
        .unwrap();
    assert!(matches!(
        builder.operation::<(), (), ()>("get", "other", OperationKind::Mutation, ""),
        Err(Error::DuplicateOperation(_))
    ));
}

#[test]
fn declaration_order_does_not_change_contract_metadata() {
    fn build(reverse: bool) -> spline_contract::ContractDescription {
        let mut builder = ContractBuilder::new("users", "docs").unwrap();
        let operations = if reverse {
            ["create", "get"]
        } else {
            ["get", "create"]
        };
        for name in operations {
            let kind = if name == "get" {
                OperationKind::Query
            } else {
                OperationKind::Mutation
            };
            builder
                .operation::<Input, String, ()>(name, name, kind, "docs")
                .unwrap();
        }
        builder.finish().unwrap()
    }
    assert_eq!(build(false), build(true));
}
