-- Subject → principal binding and principal suspension (D-05/S-03 spike).
-- The App never sees an email address or credential. Identity proves "authority X says subject S
-- presented a valid session"; this table alone decides which Vink principal that is, and the
-- suspension table decides whether the principal may act. Provider subjects are never principals.

CREATE TABLE principal_bindings (
    authority text NOT NULL CHECK (char_length(authority) BETWEEN 1 AND 200),
    subject_id text NOT NULL CHECK (char_length(subject_id) BETWEEN 1 AND 200),
    -- One subject per principal in the pilot: no flow may attach a second subject (account
    -- linking) implicitly. An explicit, verified linking flow would relax this on purpose.
    principal_id uuid NOT NULL CONSTRAINT principal_bindings_principal_key UNIQUE
        REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT principal_bindings_pkey PRIMARY KEY (authority, subject_id)
);

COMMENT ON TABLE principal_bindings IS
    'Which Vink principal an authenticated (authority, subject) acts as. Created once, atomically with the principal; never updated or deleted, so ownership cannot move to another subject.';

CREATE FUNCTION reject_principal_binding_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'principal bindings are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER principal_bindings_are_immutable
    BEFORE UPDATE OR DELETE ON principal_bindings
    FOR EACH ROW EXECUTE FUNCTION reject_principal_binding_change();

CREATE TABLE principal_suspensions (
    id uuid PRIMARY KEY,
    principal_id uuid NOT NULL REFERENCES principals (id),
    reason text NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 200 AND reason !~ '[[:cntrl:]]'),
    suspended_at timestamptz NOT NULL DEFAULT now(),
    lifted_at timestamptz,
    CONSTRAINT principal_suspensions_lift_order CHECK (lifted_at IS NULL OR lifted_at >= suspended_at)
);

-- At most one active suspension per principal; lifted ones stay as history.
CREATE UNIQUE INDEX principal_suspensions_active_key
    ON principal_suspensions (principal_id) WHERE lifted_at IS NULL;

COMMENT ON TABLE principal_suspensions IS
    'Vink community suspension (App authority, operator command only). An active row makes every request of the principal resolve without authority, whatever Identity says.';
