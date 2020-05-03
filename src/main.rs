use juniper;

#[derive(Clone)]
struct Context {}

impl juniper::Context for Context {}

type ReturnType1<'a, V: Into<juniper::Value>, E: Into<juniper::FieldError>> = juniper::BoxFuture<'a, Result<V,E>>;
type InnerReturnType = juniper::ExecutionResult<juniper::DefaultScalarValue>;
type ReturnType<'a> = juniper::BoxFuture<'a, InnerReturnType>;

type Resolver<V: Into<juniper::Value>, E: Into<juniper::FieldError>> = for<'a> fn(
    &'a juniper::Executor<Context, juniper::DefaultScalarValue>
) -> ReturnType1<'a, V, E>;

async fn to_graphql<'a, V: Into<juniper::Value>, E: Into<juniper::FieldError>>(f: ReturnType1<'a, V, E>) -> InnerReturnType {
    f.await
        .map(|scalar| scalar.into())
        .map_err(|err| err.into())
}

trait Registrable: Send + Sync
{
    fn resolve<'a>(self: &Self, executor: &'a juniper::Executor<Context, juniper::DefaultScalarValue>) -> ReturnType<'a>;
}

struct FieldInfo<S, E>
where S: Into<juniper::Value>,
    E: Into<juniper::FieldError>
{
    resolver: Resolver<S,E>
}

impl<S: Into<juniper::Value>, E: Into<juniper::FieldError>> Registrable for FieldInfo<S,E>
where S: juniper::GraphQLType<TypeInfo=()> + Send + Sync
{
    fn resolve<'a>(self: &Self, executor: &'a juniper::Executor<Context, juniper::DefaultScalarValue>) -> ReturnType<'a>
    where S: 'a, E: 'a {
        Box::pin(to_graphql((self.resolver)(executor)))
    }
}

fn main() {}
