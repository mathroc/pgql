#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub type_id: i32,
}

#[derive(Clone, Debug)]
pub struct Relation {
    pub name: String,
    pub columns: Vec<Column>,
}

impl Relation {
    pub fn from<'a>(client: &'a tokio_postgres::Transaction<'a>, oid: i32, name: String) -> juniper::BoxFuture<'a, Self> {
        Box::pin(async move {
            let query = "
                select attname, atttypid::int
                from pg_attribute
                where attrelid = $1::int::oid and attnum >= 1
            ";

            let columns = client
                .query(query, &[&oid])
                .await
                .unwrap()
                .iter()
                .map(|table| Column {
                    name: table.get("attname"),
                    type_id: table.get("atttypid"),
                })
                .collect();

            Self { name, columns }
        })
    }
}


#[derive(Clone, Debug)]
pub struct Schema {
    pub name: String,
    pub relations: Vec<Relation>,
}

impl Schema {
    pub fn from<'a>(client: &'a tokio_postgres::Transaction<'a>, name: String) -> juniper::BoxFuture<'a, Self> {
        Box::pin(async move {
            let query = "
                select oid::int
                from pg_namespace
                where nspname = $1::text
            ";

            let oid = client
                .query_opt(query, &[&name])
                .await
                .unwrap()
                .unwrap()
                .get::<_, i32>(0);

            let query = "
                with pgql_class as (
                    select oid, relname, relkind
                    from pg_class
                    where relnamespace = $1::int::oid
                ), pgql_relations as (
                    select c.oid, c.relname
                    from pgql_class c
                    where c.relkind = any(array['r', 'v'])
                )
                select oid::int, relname
                from pgql_relations
            ";

            let relations = futures::future::join_all(client
                .query(query, &[&oid])
                .await
                .unwrap()
                .iter()
                .map(|table| Relation::from(
                    client,
                    table.get("oid"),
                    table.get("relname")
                ))
                .collect::<Vec<_>>()).await;

            Self { name, relations }
        })
    }
}

#[derive(Clone, Debug)]
pub struct Database {
    name: String,
    pub schemas: Vec<Schema>,
}

impl Database {
    pub fn from<'a>(client: &'a tokio_postgres::Transaction<'a>) -> juniper::BoxFuture<'a, Self> {
        Box::pin(async move {
            let name: String = client
                .query_one("select current_database()", &[])
                .await
                .unwrap()
                .get(0);

            Self {
                name: name.clone(),
                schemas: Self::find_schemas(client, name).await,
            }
        })
    }

    fn find_schemas<'a>(
        client: &'a tokio_postgres::Transaction<'a>,
        database: String,
    ) -> juniper::BoxFuture<'a, Vec<Schema>> {
        Box::pin(async move {
            let query = "
                select description
                from pg_shdescription
                join pg_database on objoid = pg_database.oid
                where datname = $1
            ";

            let comment: String = client
                .query_opt(query, &[&database])
                .await
                .unwrap()
                .map_or("public".into(), |row| row.get(0));

            futures::future::join_all(
                comment
                    .split(',')
                    .map(|name| Schema::from(client, name.into())),
            )
            .await
        })
    }

    pub fn relations(self: &Self) -> Vec<Relation> {
        self.schemas
            .iter()
            .map(|schema| schema.relations.clone())
            .flatten()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Introspection {
    pub database: Database,
}

impl Introspection {
    pub fn from(pool: &crate::connection::Pool) -> juniper::BoxFuture<Self> {
        Box::pin(async move {
            let mut connection = pool
                .get()
                .await
                .unwrap();

            let transaction = connection
                .build_transaction()
                .read_only(true)
                .start()
                .await
                .unwrap();

            Self {
                database: Database::from(&transaction).await,
            }
        })
    }

    pub fn relations(self: &Self) -> Vec<Relation> {
        self.database.relations()
    }
}
