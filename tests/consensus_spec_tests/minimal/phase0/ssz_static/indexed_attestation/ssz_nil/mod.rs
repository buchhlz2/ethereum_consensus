// WARNING!
// This file was generated with `gen-tests`. Do NOT edit manually.

use crate::spec_test_runners::ssz_static::IndexedAttestationTestCase;
use crate::test_utils::TestCase;

#[test]
fn test_case_0() {
    let test_case = IndexedAttestationTestCase::from(
        "consensus-spec-tests/tests/minimal/phase0/ssz_static/IndexedAttestation/ssz_nil/case_0",
    );

    test_case.execute();
}