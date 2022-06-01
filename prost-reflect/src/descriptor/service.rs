use std::fmt;

use prost_types::{FileDescriptorProto, MethodDescriptorProto, ServiceDescriptorProto};

use super::{
    debug_fmt_iter, make_full_name, parse_name, parse_namespace, to_index, ty, DescriptorError,
    DescriptorPool, DescriptorPoolRef, FileDescriptor, FileDescriptorRef, FileIndex,
    MessageDescriptor, MessageDescriptorRef, MethodIndex, ServiceIndex,
};

/// A protobuf service definition.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pool: DescriptorPool,
    index: ServiceIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ServiceDescriptorRef<'a> {
    pool: DescriptorPoolRef<'a>,
    index: ServiceIndex,
}

#[derive(Clone)]
pub(super) struct ServiceDescriptorInner {
    file: FileIndex,
    full_name: Box<str>,
    methods: Box<[MethodDescriptorInner]>,
}

/// A method definition for a [`ServiceDescriptor`].
#[derive(Clone, PartialEq, Eq)]
pub struct MethodDescriptor {
    service: ServiceDescriptor,
    index: MethodIndex,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MethodDescriptorRef<'a> {
    service: ServiceDescriptorRef<'a>,
    index: MethodIndex,
}

#[derive(Clone)]
struct MethodDescriptorInner {
    full_name: Box<str>,
    request_ty: ty::TypeId,
    response_ty: ty::TypeId,
    server_streaming: bool,
    client_streaming: bool,
}

impl ServiceDescriptor {
    /// Create a new [`ServiceDescriptor`] referencing the service at `index` within the given [`DescriptorPool`].
    ///
    /// # Panics
    ///
    /// Panics if `index` is out-of-bounds.
    pub fn new(pool: DescriptorPool, index: usize) -> Self {
        ServiceDescriptorRef::new(pool.as_ref(), index).to_owned()
    }

    /// Gets a [`ServiceDescriptorRef`] referencing this service.
    pub fn as_ref(&self) -> ServiceDescriptorRef<'_> {
        ServiceDescriptorRef {
            pool: self.pool.as_ref(),
            index: self.index,
        }
    }

    /// Returns the index of this [`ServiceDescriptor`] within the parent [`DescriptorPool`].
    pub fn index(&self) -> usize {
        self.as_ref().index()
    }

    /// Gets a reference to the [`DescriptorPool`] this service is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this service is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets the short name of the service, e.g. `MyService`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the service, e.g. `my.package.Service`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets the name of the package this service is defined in, e.g. `my.package`.
    ///
    /// If no package name is set, an empty string is returned.
    pub fn package_name(&self) -> &str {
        self.as_ref().package_name()
    }

    /// Gets a reference to the raw [`ServiceDescriptorProto`] wrapped by this [`ServiceDescriptor`].
    pub fn service_descriptor_proto(&self) -> &ServiceDescriptorProto {
        self.as_ref().service_descriptor_proto()
    }

    /// Gets an iterator yielding a [`MethodDescriptor`] for each method defined in this service.
    pub fn methods(&self) -> impl ExactSizeIterator<Item = MethodDescriptor> + '_ {
        self.as_ref().methods().map(MethodDescriptorRef::to_owned)
    }
}

impl<'a> ServiceDescriptorRef<'a> {
    pub fn new(pool: DescriptorPoolRef<'a>, index: usize) -> Self {
        debug_assert!(index < pool.services().len());
        ServiceDescriptorRef {
            pool,
            index: to_index(index),
        }
    }

    pub fn to_owned(self) -> ServiceDescriptor {
        ServiceDescriptor {
            pool: self.pool.to_owned(),
            index: self.index,
        }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.pool
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        FileDescriptorRef::new(self.pool, self.inner().file as _)
    }

    pub fn name(&self) -> &'a str {
        parse_name(self.full_name())
    }

    pub fn full_name(&self) -> &'a str {
        &self.inner().full_name
    }

    pub fn package_name(&self) -> &'a str {
        parse_namespace(self.full_name())
    }

    pub fn service_descriptor_proto(&self) -> &'a ServiceDescriptorProto {
        let name = self.name();
        let package = self.package_name();
        self.parent_pool()
            .file_descriptor_protos()
            .filter(|file| file.package() == package)
            .flat_map(|file| file.service.iter())
            .find(|service| service.name() == name)
            .expect("service proto not found")
    }

    pub fn methods(&self) -> impl ExactSizeIterator<Item = MethodDescriptorRef<'a>> + 'a {
        let this = *self;
        (0..self.inner().methods.len()).map(move |index| MethodDescriptorRef::new(this, index))
    }

    fn inner(&self) -> &'a ServiceDescriptorInner {
        &self.parent_pool().inner.services[self.index as usize]
    }
}

impl ServiceDescriptorInner {
    pub(super) fn from_raw(
        raw_file: &FileDescriptorProto,
        file_index: FileIndex,
        raw_service: &ServiceDescriptorProto,
        type_map: &ty::TypeMap,
    ) -> Result<ServiceDescriptorInner, DescriptorError> {
        let full_name = make_full_name(raw_file.package(), raw_service.name());
        let methods = raw_service
            .method
            .iter()
            .map(|raw_method| {
                MethodDescriptorInner::from_raw(
                    &full_name,
                    raw_file,
                    raw_service,
                    raw_method,
                    type_map,
                )
            })
            .collect::<Result<_, DescriptorError>>()?;
        Ok(ServiceDescriptorInner {
            full_name,
            methods,
            file: file_index,
        })
    }
}

impl fmt::Debug for ServiceDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for ServiceDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("index", &self.index())
            .field("methods", &debug_fmt_iter(self.methods()))
            .finish()
    }
}

impl MethodDescriptor {
    /// Create a new [`MethodDescriptor`] referencing the method at `index` within the [`ServiceDescriptor`].
    ///
    /// # Panics
    ///
    /// Panics if `index` is out-of-bounds.
    pub fn new(service: ServiceDescriptor, index: usize) -> Self {
        MethodDescriptorRef::new(service.as_ref(), index).to_owned()
    }

    /// Gets a [`MethodDescriptorRef`] referencing this method.
    pub fn as_ref(&self) -> MethodDescriptorRef<'_> {
        MethodDescriptorRef {
            service: self.service.as_ref(),
            index: self.index,
        }
    }

    /// Gets the index of the method within the parent [`ServiceDescriptor`].
    pub fn index(&self) -> usize {
        self.as_ref().index()
    }

    /// Gets a reference to the [`ServiceDescriptor`] this method is defined in.
    pub fn parent_service(&self) -> ServiceDescriptor {
        self.as_ref().parent_service().to_owned()
    }

    /// Gets a reference to the [`DescriptorPool`] this method is defined in.
    pub fn parent_pool(&self) -> DescriptorPool {
        self.as_ref().parent_pool().to_owned()
    }

    /// Gets the [`FileDescriptor`] this method is defined in.
    pub fn parent_file(&self) -> FileDescriptor {
        self.as_ref().parent_file().to_owned()
    }

    /// Gets the short name of the method, e.g. `method`.
    pub fn name(&self) -> &str {
        self.as_ref().name()
    }

    /// Gets the full name of the method, e.g. `my.package.MyService.my_method`.
    pub fn full_name(&self) -> &str {
        self.as_ref().full_name()
    }

    /// Gets a reference to the raw [`MethodDescriptorProto`] wrapped by this [`MethodDescriptor`].
    pub fn method_descriptor_proto(&self) -> &MethodDescriptorProto {
        self.as_ref().method_descriptor_proto()
    }

    /// Gets the [`MessageDescriptor`] for the input type of this method.
    pub fn input(&self) -> MessageDescriptor {
        self.as_ref().input().to_owned()
    }

    /// Gets the [`MessageDescriptor`] for the output type of this method.
    pub fn output(&self) -> MessageDescriptor {
        self.as_ref().output().to_owned()
    }

    /// Returns `true` if the client streams multiple messages.
    pub fn is_client_streaming(&self) -> bool {
        self.as_ref().is_client_streaming()
    }

    /// Returns `true` if the server streams multiple messages.
    pub fn is_server_streaming(&self) -> bool {
        self.as_ref().is_server_streaming()
    }
}

impl<'a> MethodDescriptorRef<'a> {
    pub fn new(service: ServiceDescriptorRef<'a>, index: usize) -> Self {
        debug_assert!(index < service.methods().len());
        MethodDescriptorRef {
            service,
            index: to_index(index),
        }
    }

    pub fn to_owned(self) -> MethodDescriptor {
        MethodDescriptor {
            service: self.service.to_owned(),
            index: self.index,
        }
    }

    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub fn parent_service(&self) -> ServiceDescriptorRef<'a> {
        self.service
    }

    pub fn parent_pool(&self) -> DescriptorPoolRef<'a> {
        self.service.parent_pool()
    }

    pub fn parent_file(&self) -> FileDescriptorRef<'a> {
        self.service.parent_file()
    }

    pub fn name(&self) -> &'a str {
        parse_name(self.full_name())
    }

    pub fn full_name(&self) -> &'a str {
        &self.inner().full_name
    }

    pub fn method_descriptor_proto(&self) -> &'a MethodDescriptorProto {
        &self.parent_service().service_descriptor_proto().method[self.index as usize]
    }

    pub fn input(&self) -> MessageDescriptorRef<'a> {
        MessageDescriptorRef::new(self.parent_pool(), self.inner().request_ty)
    }

    pub fn output(&self) -> MessageDescriptorRef<'a> {
        MessageDescriptorRef::new(self.parent_pool(), self.inner().response_ty)
    }

    pub fn is_client_streaming(&self) -> bool {
        self.inner().client_streaming
    }

    pub fn is_server_streaming(&self) -> bool {
        self.inner().server_streaming
    }

    fn inner(&self) -> &'a MethodDescriptorInner {
        &self.service.inner().methods[self.index as usize]
    }
}

impl MethodDescriptorInner {
    fn from_raw(
        namespace: &str,
        _raw_file: &FileDescriptorProto,
        _raw_service: &ServiceDescriptorProto,
        raw_method: &MethodDescriptorProto,
        type_map: &ty::TypeMap,
    ) -> Result<MethodDescriptorInner, DescriptorError> {
        let full_name = make_full_name(namespace, raw_method.name());

        let request_ty = type_map.resolve_type_name(namespace, raw_method.input_type())?;
        if !request_ty.is_message() {
            return Err(DescriptorError::invalid_method_type(
                full_name,
                raw_method.input_type(),
            ));
        }

        let response_ty = type_map.resolve_type_name(namespace, raw_method.output_type())?;
        if !response_ty.is_message() {
            return Err(DescriptorError::invalid_method_type(
                full_name,
                raw_method.output_type(),
            ));
        }

        Ok(MethodDescriptorInner {
            full_name,
            request_ty,
            response_ty,
            client_streaming: raw_method.client_streaming(),
            server_streaming: raw_method.server_streaming(),
        })
    }
}

impl fmt::Debug for MethodDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for MethodDescriptorRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodDescriptor")
            .field("name", &self.name())
            .field("full_name", &self.full_name())
            .field("index", &self.index())
            .field("input", &self.input())
            .field("output", &self.output())
            .field("is_client_streaming", &self.is_client_streaming())
            .field("is_server_streaming", &self.is_server_streaming())
            .finish()
    }
}
