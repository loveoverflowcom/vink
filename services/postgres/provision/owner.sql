-- Provisions one Vink PostgreSQL owner: a UTF8 database, its migrator role and its runtime role.
--
-- Run through ../provision.sh as the bootstrap/operator superuser, connected to a maintenance
-- database such as `postgres`. Idempotent and additive: it creates what is missing, adopts
-- objects the operator owns in schema `public`, and never changes an existing role or password,
-- never drops anything. Every run ends with read-only post-checks; `check_only` runs only checks.
--
-- psql variables: db, migrator, runtime, history_table, check_only (0 provision, 1 check).
-- Environment, read only when a role has to be created (plain text or a SCRAM verifier):
--   VINK_PROVISION_MIGRATOR_PASSWORD, VINK_PROVISION_RUNTIME_PASSWORD
-- Needs psql 15+ (\getenv). CREATE DATABASE cannot run in a transaction, so a failure part-way
-- leaves earlier steps applied; rerunning completes them.

\set ON_ERROR_STOP on
\set QUIET on
\set admin_db :DBNAME

SELECT set_config('vink_provision.db', :'db', false),
       set_config('vink_provision.migrator', :'migrator', false),
       set_config('vink_provision.runtime', :'runtime', false),
       set_config('vink_provision.history', :'history_table', false) \gset prov_

-- 1. Cluster pre-checks (read-only).
DO $precheck$
DECLARE
    db       text := current_setting('vink_provision.db');
    migrator text := current_setting('vink_provision.migrator');
    runtime  text := current_setting('vink_provision.runtime');
    history  text := current_setting('vink_provision.history');
    item     text;
    r        record;
BEGIN
    FOREACH item IN ARRAY ARRAY[db, migrator, runtime, history] LOOP
        IF item !~ '^[a-z_][a-z0-9_]{0,62}$' THEN
            RAISE EXCEPTION 'invalid name "%": use lower-case letters, digits and _', item;
        END IF;
    END LOOP;
    IF migrator = runtime THEN
        RAISE EXCEPTION 'migrator and runtime must be different roles (both are %)', runtime;
    END IF;
    IF NOT (SELECT rolsuper FROM pg_roles WHERE rolname = current_user) THEN
        RAISE EXCEPTION 'connected as %, which is not a superuser; use the bootstrap/operator credential',
            current_user;
    END IF;
    IF current_user IN (migrator, runtime) THEN
        RAISE EXCEPTION 'the operator role % cannot also be a service role', current_user;
    END IF;
    IF db IN ('postgres', 'template0', 'template1') THEN
        RAISE EXCEPTION 'database % is reserved; choose an owner database', db;
    END IF;
    FOR r IN SELECT * FROM pg_roles WHERE rolname IN (migrator, runtime) LOOP
        IF r.rolsuper OR r.rolcreatedb OR r.rolcreaterole OR r.rolreplication OR r.rolbypassrls THEN
            RAISE EXCEPTION 'role % exists with elevated attributes and provisioning never edits roles; review it, then ALTER ROLE % NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS',
                r.rolname, quote_ident(r.rolname);
        END IF;
        IF NOT r.rolcanlogin THEN
            RAISE EXCEPTION 'role % exists without LOGIN', r.rolname;
        END IF;
    END LOOP;
    SELECT string_agg(format('%s in %s', m.rolname, g.rolname), ', ') INTO item
      FROM pg_auth_members a
      JOIN pg_roles m ON m.oid = a.member
      JOIN pg_roles g ON g.oid = a.roleid
     WHERE m.rolname IN (migrator, runtime);
    IF item IS NOT NULL THEN
        RAISE EXCEPTION 'service roles must not be members of other roles (found %)', item;
    END IF;
    SELECT o.rolname INTO item
      FROM pg_database d JOIN pg_roles o ON o.oid = d.datdba
     WHERE d.datname = db;
    IF item IN (migrator, runtime) THEN
        RAISE EXCEPTION 'database % is owned by service role %; the operator must own it', db, item;
    END IF;
    IF EXISTS (SELECT FROM pg_database WHERE datname = db AND pg_encoding_to_char(encoding) <> 'UTF8') THEN
        RAISE EXCEPTION 'database % is not UTF8', db;
    END IF;
END
$precheck$;

SELECT EXISTS (SELECT FROM pg_database WHERE datname = :'db') AS db_exists \gset

-- 2. Existing database pre-checks (read-only): nothing is adopted unless all of it can be.
\if :db_exists
\connect :db
SELECT set_config('vink_provision.migrator', :'migrator', false) \gset prov_
DO $existing$
DECLARE
    migrator oid := (SELECT oid FROM pg_roles WHERE rolname = current_setting('vink_provision.migrator'));
    operator oid := (SELECT oid FROM pg_roles WHERE rolname = current_user);
    item     text;
BEGIN
    SELECT string_agg(extname, ', ') INTO item
      FROM pg_extension WHERE extname IN ('dblink', 'postgres_fdw');
    IF item IS NOT NULL THEN
        RAISE EXCEPTION 'database % has cross-database extensions (%); remove them deliberately first',
            current_database(), item;
    END IF;
    SELECT string_agg(c.oid::regclass::text, ', ') INTO item
      FROM pg_class c WHERE c.relnamespace = 'public'::regnamespace AND c.relkind = 'f';
    IF item IS NOT NULL THEN
        RAISE EXCEPTION 'database % has foreign tables (%); remove them deliberately first',
            current_database(), item;
    END IF;
    SELECT string_agg(format('%s (owner %s)', name, owner::regrole), ', ') INTO item
      FROM (
            SELECT c.oid::regclass::text AS name, c.relowner AS owner
              FROM pg_class c
             WHERE c.relnamespace = 'public'::regnamespace
               AND c.relkind IN ('r', 'p', 'v', 'm', 'S', 'c')
               AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass
                                  AND d.objid = c.oid AND d.deptype = 'e')
            UNION ALL
            SELECT p.oid::regprocedure::text, p.proowner
              FROM pg_proc p
             WHERE p.pronamespace = 'public'::regnamespace
               AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_proc'::regclass
                                  AND d.objid = p.oid AND d.deptype = 'e')
            UNION ALL
            SELECT t.oid::regtype::text, t.typowner
              FROM pg_type t
             WHERE t.typnamespace = 'public'::regnamespace AND t.typtype IN ('d', 'e', 'r')
               AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_type'::regclass
                                  AND d.objid = t.oid AND d.deptype = 'e')
           ) objects
     WHERE owner <> operator AND owner IS DISTINCT FROM migrator;
    IF item IS NOT NULL THEN
        RAISE EXCEPTION 'objects in %.public belong to neither the operator nor the migrator: %; reassign them deliberately first',
            current_database(), item;
    END IF;
END
$existing$;
\connect :admin_db
\endif

\if :check_only
\if :db_exists
\else
DO $missing$ BEGIN
    RAISE EXCEPTION 'database % does not exist', current_setting('vink_provision.db');
END $missing$;
\endif
\else

-- 3. Roles: created only when missing, least-privileged, password never changed afterwards.
\getenv migrator_password VINK_PROVISION_MIGRATOR_PASSWORD
\getenv runtime_password VINK_PROVISION_RUNTIME_PASSWORD
SELECT NOT EXISTS (SELECT FROM pg_roles WHERE rolname = :'migrator') AS create_migrator,
       NOT EXISTS (SELECT FROM pg_roles WHERE rolname = :'runtime') AS create_runtime \gset
\if :create_migrator
\if :{?migrator_password}
SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD %L',
              :'migrator', :'migrator_password') \gexec
\echo created role :migrator
\else
DO $nopw$ BEGIN
    RAISE EXCEPTION 'role % does not exist and VINK_PROVISION_MIGRATOR_PASSWORD is not set',
        current_setting('vink_provision.migrator');
END $nopw$;
\endif
\endif
\if :create_runtime
\if :{?runtime_password}
SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD %L',
              :'runtime', :'runtime_password') \gexec
\echo created role :runtime
\else
DO $nopw$ BEGIN
    RAISE EXCEPTION 'role % does not exist and VINK_PROVISION_RUNTIME_PASSWORD is not set',
        current_setting('vink_provision.runtime');
END $nopw$;
\endif
\endif
\unset migrator_password
\unset runtime_password

-- 4. Database: created when missing, owned by the operator, UTF8 for Vietnamese text.
\if :db_exists
\else
SELECT format('CREATE DATABASE %I WITH TEMPLATE template0 ENCODING %L', :'db', 'UTF8') \gexec
\echo created database :db
\endif

-- 5. Privileges inside the owner database, in one transaction.
\connect :db
SELECT set_config('vink_provision.migrator', :'migrator', false),
       set_config('vink_provision.history', :'history_table', false) \gset prov_
BEGIN;
REVOKE CONNECT, TEMPORARY, CREATE ON DATABASE :"db" FROM PUBLIC;
GRANT CONNECT ON DATABASE :"db" TO :"migrator", :"runtime";
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO :"migrator";
GRANT USAGE ON SCHEMA public TO :"runtime";

-- Objects the operator created (the former single-credential `migrate`) move to the migrator,
-- which future migrations need to ALTER them. Sequences owned by a column follow their table.
DO $adopt$
DECLARE
    migrator text := current_setting('vink_provision.migrator');
    operator oid := (SELECT oid FROM pg_roles WHERE rolname = current_user);
    o        record;
BEGIN
    FOR o IN
        SELECT CASE c.relkind WHEN 'v' THEN 'VIEW' WHEN 'm' THEN 'MATERIALIZED VIEW'
                              WHEN 'S' THEN 'SEQUENCE' WHEN 'c' THEN 'TYPE' ELSE 'TABLE' END AS kind,
               c.oid::regclass::text AS name
          FROM pg_class c
         WHERE c.relnamespace = 'public'::regnamespace AND c.relowner = operator
           AND c.relkind IN ('r', 'p', 'v', 'm', 'S', 'c')
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass
                              AND d.objid = c.oid AND d.deptype IN ('e', 'a', 'i'))
        UNION ALL
        SELECT CASE p.prokind WHEN 'p' THEN 'PROCEDURE' WHEN 'a' THEN 'AGGREGATE' ELSE 'FUNCTION' END,
               p.oid::regprocedure::text
          FROM pg_proc p
         WHERE p.pronamespace = 'public'::regnamespace AND p.proowner = operator
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_proc'::regclass
                              AND d.objid = p.oid AND d.deptype = 'e')
        UNION ALL
        SELECT CASE t.typtype WHEN 'd' THEN 'DOMAIN' ELSE 'TYPE' END, t.oid::regtype::text
          FROM pg_type t
         WHERE t.typnamespace = 'public'::regnamespace AND t.typowner = operator
           AND t.typtype IN ('d', 'e', 'r')
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_type'::regclass
                              AND d.objid = t.oid AND d.deptype = 'e')
    LOOP
        RAISE NOTICE 'adopting % % for %', o.kind, o.name, migrator;
        EXECUTE format('ALTER %s %s OWNER TO %I', o.kind, o.name, migrator);
    END LOOP;
END
$adopt$;

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO :"runtime";
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO :"runtime";
ALTER DEFAULT PRIVILEGES FOR ROLE :"migrator" IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO :"runtime";
ALTER DEFAULT PRIVILEGES FOR ROLE :"migrator" IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO :"runtime";

-- The migration history is written only by its owner; vink-store-postgres repeats this after
-- every migration run, because the first run creates the table under the defaults above.
DO $history$
DECLARE
    history regclass := to_regclass(format('public.%I', current_setting('vink_provision.history')));
    grantee text;
BEGIN
    IF history IS NULL THEN
        RETURN;
    END IF;
    FOR grantee IN
        SELECT DISTINCT CASE WHEN a.grantee = 0 THEN 'PUBLIC' ELSE quote_ident(r.rolname) END
          FROM pg_class c
          CROSS JOIN LATERAL aclexplode(c.relacl) a
          LEFT JOIN pg_roles r ON r.oid = a.grantee
         WHERE c.oid = history AND a.grantee <> c.relowner
           AND a.privilege_type IN ('INSERT', 'UPDATE', 'DELETE', 'TRUNCATE')
    LOOP
        EXECUTE format('REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON TABLE %s FROM %s', history, grantee);
    END LOOP;
END
$history$;
COMMIT;
\connect :admin_db
\endif

-- 6. Post-checks (read-only), run in the owner database.
\connect :db
SELECT set_config('vink_provision.db', :'db', false),
       set_config('vink_provision.migrator', :'migrator', false),
       set_config('vink_provision.runtime', :'runtime', false),
       set_config('vink_provision.history', :'history_table', false) \gset prov_
DO $postcheck$
DECLARE
    db         text := current_setting('vink_provision.db');
    migrator   text := current_setting('vink_provision.migrator');
    runtime    text := current_setting('vink_provision.runtime');
    history    regclass := to_regclass(format('public.%I', current_setting('vink_provision.history')));
    role_name  text;
    priv       text;
    item       text;
    r          record;
    problems   text[] := '{}';
BEGIN
    FOREACH role_name IN ARRAY ARRAY[migrator, runtime] LOOP
        SELECT * INTO r FROM pg_roles WHERE rolname = role_name;
        IF NOT FOUND THEN
            problems := problems || format('role %s is missing', role_name);
            CONTINUE;
        END IF;
        IF r.rolsuper OR r.rolcreatedb OR r.rolcreaterole OR r.rolreplication OR r.rolbypassrls
           OR NOT r.rolcanlogin THEN
            problems := problems || format('role %s must be LOGIN without SUPERUSER/CREATEDB/CREATEROLE/REPLICATION/BYPASSRLS', role_name);
        END IF;
        IF EXISTS (SELECT FROM pg_auth_members a WHERE a.member = r.oid) THEN
            problems := problems || format('role %s is a member of another role', role_name);
        END IF;
        IF NOT has_database_privilege(role_name, db, 'CONNECT') THEN
            problems := problems || format('%s lacks CONNECT on %s', role_name, db);
        END IF;
        FOREACH priv IN ARRAY ARRAY['CREATE', 'TEMPORARY'] LOOP
            IF has_database_privilege(role_name, db, priv) THEN
                problems := problems || format('%s has %s on database %s', role_name, priv, db);
            END IF;
        END LOOP;
        SELECT string_agg(d.datname, ', ') INTO item
          FROM pg_database d
          CROSS JOIN LATERAL aclexplode(coalesce(d.datacl, acldefault('d', d.datdba))) a
         WHERE d.datname <> db AND a.grantee = r.oid;
        IF item IS NOT NULL THEN
            problems := problems || format('%s holds explicit privileges on other databases: %s', role_name, item);
        END IF;
    END LOOP;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = migrator)
       OR NOT EXISTS (SELECT FROM pg_roles WHERE rolname = runtime) THEN
        RAISE EXCEPTION 'post-check failed for database %: %', db, array_to_string(problems, '; ');
    END IF;

    IF pg_encoding_to_char((SELECT encoding FROM pg_database WHERE datname = db)) <> 'UTF8' THEN
        problems := problems || format('database %s is not UTF8', db);
    END IF;
    IF EXISTS (SELECT FROM pg_database d
                 CROSS JOIN LATERAL aclexplode(coalesce(d.datacl, acldefault('d', d.datdba))) a
                WHERE d.datname = db AND a.grantee = 0) THEN
        problems := problems || format('PUBLIC still has privileges on database %s', db);
    END IF;
    IF EXISTS (SELECT FROM pg_namespace n
                 CROSS JOIN LATERAL aclexplode(coalesce(n.nspacl, acldefault('n', n.nspowner))) a
                WHERE n.nspname = 'public' AND a.grantee = 0 AND a.privilege_type = 'CREATE') THEN
        problems := problems || 'PUBLIC can CREATE in schema public';
    END IF;
    IF NOT has_schema_privilege(migrator, 'public', 'CREATE') THEN
        problems := problems || format('%s cannot CREATE in schema public', migrator);
    END IF;
    IF has_schema_privilege(runtime, 'public', 'CREATE') OR NOT has_schema_privilege(runtime, 'public', 'USAGE') THEN
        problems := problems || format('%s must have USAGE without CREATE on schema public', runtime);
    END IF;
    IF EXISTS (SELECT FROM pg_extension WHERE extname IN ('dblink', 'postgres_fdw'))
       OR EXISTS (SELECT FROM pg_class WHERE relkind = 'f') THEN
        problems := problems || 'cross-database extensions or foreign tables are present';
    END IF;

    SELECT string_agg(name, ', ') INTO item FROM (
        SELECT c.oid::regclass::text AS name FROM pg_class c
         WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'p', 'v', 'm', 'S', 'c')
           AND c.relowner <> migrator::regrole
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_class'::regclass
                              AND d.objid = c.oid AND d.deptype = 'e')
        UNION ALL
        SELECT p.oid::regprocedure::text FROM pg_proc p
         WHERE p.pronamespace = 'public'::regnamespace AND p.proowner <> migrator::regrole
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_proc'::regclass
                              AND d.objid = p.oid AND d.deptype = 'e')
        UNION ALL
        SELECT t.oid::regtype::text FROM pg_type t
         WHERE t.typnamespace = 'public'::regnamespace AND t.typtype IN ('d', 'e', 'r')
           AND t.typowner <> migrator::regrole
           AND NOT EXISTS (SELECT FROM pg_depend d WHERE d.classid = 'pg_type'::regclass
                              AND d.objid = t.oid AND d.deptype = 'e')
    ) foreign_owned;
    IF item IS NOT NULL THEN
        problems := problems || format('not owned by %s: %s', migrator, item);
    END IF;

    FOR r IN SELECT c.oid FROM pg_class c
              WHERE c.relnamespace = 'public'::regnamespace AND c.relkind IN ('r', 'p') LOOP
        IF r.oid = history THEN
            IF NOT has_table_privilege(runtime, r.oid, 'SELECT') THEN
                problems := problems || format('%s cannot read %s', runtime, history);
            END IF;
            FOREACH priv IN ARRAY ARRAY['INSERT', 'UPDATE', 'DELETE', 'TRUNCATE'] LOOP
                IF has_table_privilege(runtime, r.oid, priv) THEN
                    problems := problems || format('%s has %s on %s', runtime, priv, history);
                END IF;
            END LOOP;
            IF EXISTS (SELECT FROM pg_class c CROSS JOIN LATERAL aclexplode(c.relacl) a
                        WHERE c.oid = history AND a.grantee <> c.relowner
                          AND a.privilege_type IN ('INSERT', 'UPDATE', 'DELETE', 'TRUNCATE')) THEN
                problems := problems || format('%s is writable by a role other than its owner', history);
            END IF;
            CONTINUE;
        END IF;
        FOREACH priv IN ARRAY ARRAY['SELECT', 'INSERT', 'UPDATE', 'DELETE'] LOOP
            IF NOT has_table_privilege(runtime, r.oid, priv) THEN
                problems := problems || format('%s lacks %s on %s', runtime, priv, r.oid::regclass);
            END IF;
        END LOOP;
        FOREACH priv IN ARRAY ARRAY['TRUNCATE', 'REFERENCES', 'TRIGGER'] LOOP
            IF has_table_privilege(runtime, r.oid, priv) THEN
                problems := problems || format('%s has %s on %s', runtime, priv, r.oid::regclass);
            END IF;
        END LOOP;
    END LOOP;

    IF NOT EXISTS (
        SELECT FROM pg_default_acl d CROSS JOIN LATERAL aclexplode(d.defaclacl) a
         WHERE d.defaclrole = migrator::regrole AND d.defaclnamespace = 'public'::regnamespace
           AND d.defaclobjtype = 'r' AND a.grantee = runtime::regrole AND a.privilege_type = 'DELETE') THEN
        problems := problems || format('default privileges for tables created by %s are missing', migrator);
    END IF;

    IF cardinality(problems) > 0 THEN
        RAISE EXCEPTION 'post-check failed for database %: %', db, array_to_string(problems, '; ');
    END IF;

    SELECT string_agg(d.datname, ', ' ORDER BY d.datname) INTO item
      FROM pg_database d
      CROSS JOIN LATERAL aclexplode(coalesce(d.datacl, acldefault('d', d.datdba))) a
     WHERE d.datname <> db AND d.datallowconn AND a.grantee = 0 AND a.privilege_type = 'CONNECT';
    IF item IS NOT NULL THEN
        RAISE NOTICE 'databases still open to PUBLIC CONNECT (no Vink owner data expected): %', item;
    END IF;
END
$postcheck$;
\echo owner :db ok (migrator :migrator, runtime :runtime)
