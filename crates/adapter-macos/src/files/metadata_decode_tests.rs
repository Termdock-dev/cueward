use super::*;

#[test]
fn native_decoders_keep_false_empty_and_unavailable_distinct() {
    let no = NSNumber::numberWithBool(false);
    assert!(!boolean(&no).unwrap());
    let empty = NSArray::<AnyObject>::from_slice(&[]);
    assert!(tags(&empty).unwrap().is_empty());
    assert_eq!(
        decode_resource::<bool>(Ok(None), boolean),
        ResourceValue::Unavailable
    );
    // SAFETY: NSNumber is an Objective-C object; AnyObject imposes no narrower type.
    let value: Retained<AnyObject> = unsafe { Retained::cast_unchecked(no) };
    assert_eq!(
        decode_resource(Ok(Some(value)), boolean),
        ResourceValue::Available { value: false }
    );
}

#[test]
fn native_decoders_reject_wrong_types_and_oversized_tag_results() {
    let text = NSString::from_str("not a number");
    assert!(boolean(&text).is_err());
    assert!(content_type(&text).is_err());
    assert!(tags(&text).is_err());
    let number = NSNumber::numberWithInt(42);
    let wrong_tags = NSArray::<AnyObject>::from_slice(&[&number]);
    assert!(tags(&wrong_tags).is_err());
    let refs: Vec<&AnyObject> = (0..=MAX_TAGS).map(|_| &*text as &AnyObject).collect();
    assert!(tags(&NSArray::from_slice(&refs)).is_err());
    let long = NSString::from_str(&"x".repeat(MAX_TEXT_BYTES + 1));
    assert!(string(&long).is_err());
    let part = NSString::from_str(&"x".repeat(MAX_TEXT_BYTES / 2 + 1));
    assert!(tags(&NSArray::<AnyObject>::from_slice(&[&part, &part])).is_err());
}

#[test]
fn resource_error_keeps_native_domain_code_and_message() {
    // SAFETY: There is no userInfo dictionary to violate the generic value contract.
    let error = unsafe {
        NSError::errorWithDomain_code_userInfo(&NSString::from_str("test.metadata"), 77, None)
    };
    let message = error.localizedDescription().to_string();
    let value: ResourceValue<bool> = decode_resource(Err(error), boolean);
    assert_eq!(
        value,
        ResourceValue::Error {
            error: MetadataError {
                domain: "test.metadata".into(),
                code: 77,
                message
            }
        }
    );
}
