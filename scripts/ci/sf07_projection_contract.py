"""Independent identity for the workflow before the SF07 source layers.

This is an audited fixed expectation, never a digest derived from the inverse
under test. The baseline includes the six Windows scratch-directory prefixes
and the two original-budget test inventory counts changing from12 to14.
Update this contract only when a reviewed baseline change is intentional.
Collector/source identities remain independent contracts.
"""

BASE_WORKFLOW_SHA256 = "4c30a985d983401c9b61ee1a3ca4edbde3ad3e924f64baed0c588eb744bae2a1"
