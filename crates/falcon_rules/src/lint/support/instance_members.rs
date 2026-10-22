use falcon_analyze::{ResolvedType, SemanticModel, SignatureIndex};

pub(crate) fn has_instance_member(
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    receiver: &ResolvedType,
    name: &str,
) -> bool {
    let receiver = match receiver {
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        } => bound.as_ref(),
        receiver => receiver,
    };
    if !matches!(
        receiver,
        ResolvedType::Interface {
            nullable: false,
            ..
        } | ResolvedType::Record {
            nullable: false,
            ..
        }
    ) {
        return false;
    }
    receiver.record_field(name).is_some()
        || signatures
            .resolved_member_owner(receiver, name, model)
            .is_some()
        || signatures.resolved_field(receiver, name, model).is_some()
        || signatures
            .resolved_writable_field(receiver, name, model)
            .is_some()
}
