//! What the connected role may do beyond reading and writing rows.

use sqlx::postgres::PgPool;

/// True when the connected role is a superuser, may create roles or databases, replicate or
/// bypass row security, or may create objects in the current database or its `public` schema.
/// A serving process holding any of these runs with a migration or operator credential.
pub async fn role_is_privileged(pool: &PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT r.rolsuper OR r.rolcreatedb OR r.rolcreaterole OR r.rolreplication OR r.rolbypassrls
                OR has_database_privilege(current_database(), 'CREATE')
                OR coalesce((SELECT has_schema_privilege(n.oid, 'CREATE')
                               FROM pg_namespace n WHERE n.nspname = 'public'), false)
           FROM pg_roles r WHERE r.rolname = current_user",
    )
    .fetch_one(pool)
    .await
}
