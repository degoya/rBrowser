use super::*;

#[test]
fn rfc_3339_times_are_read_with_fractions_and_offsets() {
    assert_eq!(parse_time("2026-11-12T00:00:00Z"), Some(1_794_441_600));
    assert_eq!(parse_time("2000-02-29t12:30:00.250z"), Some(951_827_400));
    assert_eq!(parse_time("2026-11-12T02:00:00+02:00"), Some(1_794_441_600));
    assert_eq!(parse_time("1969-12-31 23:59:59Z"), Some(-1));
    for bad in [
        "2026-11-12",
        "2026-11-12T00:00:00",
        "2026-02-30T00:00:00Z",
        "2026-13-01T00:00:00Z",
        "2026-11-12T24:00:00Z",
        "2026-11-12T00:00:00.Z",
        "2026-11-12T00:00:00+2:00",
        "Tue, 12 Nov 2026 00:00:00 GMT",
    ] {
        assert_eq!(parse_time(bad), None, "{bad}");
    }
}

#[test]
fn fields_are_read_case_blind_without_comments() {
    let fields = fields(
        "# Our policy\ncontact: mailto:security@shop.test\nContact: https://shop.test/security\n\
Expires: 2027-06-30T00:00:00Z\nPolicy: https://shop.test/policy\n",
    );
    assert_eq!(
        fields.contact,
        ["mailto:security@shop.test", "https://shop.test/security"]
    );
    assert_eq!(fields.expires, ["2027-06-30T00:00:00Z"]);
}

#[test]
fn a_signed_file_is_read_between_its_header_and_signature() {
    let fields = fields(
        "-----BEGIN PGP SIGNED MESSAGE-----\nHash: SHA512\n\nContact: mailto:a@shop.test\n\
- Expires: 2027-06-30T00:00:00Z\n-----BEGIN PGP SIGNATURE-----\nContact: mailto:forged@x.test\n\
-----END PGP SIGNATURE-----\n",
    );
    assert_eq!(fields.contact, ["mailto:a@shop.test"]);
    assert_eq!(fields.expires, ["2027-06-30T00:00:00Z"]);
}
