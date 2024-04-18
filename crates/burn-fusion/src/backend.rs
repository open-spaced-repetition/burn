use crate::{
    client::FusionClient,
    stream::{Context, OperationDescription},
    FusionClientLocator, FusionTensor, PrecisionBridge,
};
use burn_tensor::{backend::Backend, Device, Shape};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

pub(crate) static CLIENTS: FusionClientLocator = FusionClientLocator::new();

pub(crate) fn get_client<B: FusionBackend>(device: &B::FusionDevice) -> B::FusionClient {
    CLIENTS.client(device)
}

/// Enable dynamic operation fusion on a backend that implements [fusion backend](crate::FusionBackend).
#[derive(Clone, Debug, Default)]
pub struct Fusion<B> {
    _backend: PhantomData<B>,
}

impl<B: FusionBackend> Backend for Fusion<B> {
    type Device = B::Device;

    type FullPrecisionBridge = PrecisionBridge;

    type FloatTensorPrimitive<const D: usize> = FusionTensor<B::FusionClient>;

    type FloatElem = B::FloatElem;

    type IntTensorPrimitive<const D: usize> = FusionTensor<B::FusionClient>;

    type IntElem = B::IntElem;

    type BoolTensorPrimitive<const D: usize> = FusionTensor<B::FusionClient>;

    fn name() -> String {
        format!("fusion<{}>", B::name())
    }

    fn seed(seed: u64) {
        B::seed(seed);
    }

    fn sync(device: &Self::Device) {
        let client = CLIENTS.client::<B::FusionClient>(&device.clone().into());
        client.drain();
        B::sync(device)
    }
}

/// The status of a [builder](OptimizationBuilder).
#[derive(Clone, Debug, Copy)]
pub enum OptimizationStatus {
    /// No more operations can be fused.
    Closed,
    /// More operations can be fused.
    Open,
}

/// The properties of a [builder](OptimizationProperties).
#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizationProperties {
    /// The score of the optimization, higher is better.
    pub score: u64,
    /// If the operation is ready to be executed.
    pub ready: bool,
}

/// The fusion operation abstraction allows implementations to fuse many
/// [tensor operations](OperationDescription) into one, improving the performance of the backend.
///
///
/// # Notes
///
/// The implementations are free to execute the registered operations the way they want to improve
/// the speed and efficiency of the computational graph. It doesn't mean that all registered
/// operations should be fused, but that another way of executing them is more efficient.
///
/// Also, it is important to return (OptimizationStatus::Closed) when no more registered operation can
/// improve the performance.
pub trait OptimizationBuilder<O>: Send {
    /// Register a new [tensor operation](OperationDescription).
    fn register(&mut self, operation: &OperationDescription);
    /// Finish the optimization and create a fusion operation.
    fn build(&self) -> O;
    /// Reset the state.
    fn reset(&mut self);
    /// Return the builder [status](OptimizationStatus).
    fn status(&self) -> OptimizationStatus;
    /// Return the builder [properties](OptimizationProperties).
    fn properties(&self) -> OptimizationProperties;
    /// The number of operation fused.
    fn len(&self) -> usize;
    /// If no operations are fused.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The operation created from the [builder](OptimizationBuilder).
pub trait Optimization<B: FusionBackend>: Send {
    /// Execute the operation.
    fn execute(&mut self, context: &mut Context<'_, B>);
    /// The number of registered operations in this optimization.
    fn len(&self) -> usize;
    /// If the current optimization is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Returns the state that can be serialized.
    fn to_state(&self) -> B::OptimizationState;
    /// Create the optimization from the state.
    fn from_state(device: &B::Device, state: B::OptimizationState) -> Self;
}

/// The device id.
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy, new)]
pub struct DeviceId {
    /// The type id identifies the type of the device.
    pub type_id: u16,
    /// The index id identifies the device number.
    pub index_id: u32,
}

/// The handle device trait allows to get an id for a backend device.
pub trait FusionDevice: Clone + Send + Sync + PartialEq {
    /// Return the [device id](DeviceId).
    fn id(&self) -> DeviceId;
}

/// Trait that allows an existing [backend](Backend) to specify graph optimizations using
/// [operation builder](crate::OptimizationBuilder).
pub trait FusionBackend: Backend {
    /// The state that can be serialized for an optimization.
    type OptimizationState: Serialize + DeserializeOwned;
    /// Optimization type for the backend.
    type Optimization: Optimization<Self>;

    /// The device type that can return an ID.
    ///
    /// It can be the same as (Backend::Device), but must implement (FusionDevice).
    type FusionDevice: FusionDevice + From<Self::Device> + Into<Self::Device> + core::fmt::Debug;
    /// The type that can be used to point to a tensor of any kind.
    type Handle: Sync + Send + Clone;
    /// What kind of client should be used.
    type FusionClient: FusionClient<FusionBackend = Self>;

    /// The list of optimizations that will be used to optimize the computational graph.
    fn optimizations(device: Device<Self>)
        -> Vec<Box<dyn OptimizationBuilder<Self::Optimization>>>;

    /// Convert a [handle](FusionBackend::Handle) to a [float tensor](Backend::FloatTensorPrimitive).
    fn float_tensor<const D: usize>(
        handle: Self::Handle,
        shape: Shape<D>,
    ) -> Self::FloatTensorPrimitive<D>;
    /// Convert a [handle](FusionBackend::Handle) to an [int tensor](Backend::IntTensorPrimitive).
    fn int_tensor<const D: usize>(
        handle: Self::Handle,
        shape: Shape<D>,
    ) -> Self::IntTensorPrimitive<D>;
    /// Convert a [handle](FusionBackend::Handle) to a [bool tensor](Backend::BoolTensorPrimitive).
    fn bool_tensor<const D: usize>(
        handle: Self::Handle,
        shape: Shape<D>,
    ) -> Self::BoolTensorPrimitive<D>;

    /// Convert a [float tensor](Backend::FloatTensorPrimitive) to a [handle](FusionBackend::Handle).
    fn float_tensor_handle<const D: usize>(tensor: Self::FloatTensorPrimitive<D>) -> Self::Handle;
    /// Convert an [int tensor](Backend::IntTensorPrimitive) to a [handle](FusionBackend::Handle).
    fn int_tensor_handle<const D: usize>(tensor: Self::IntTensorPrimitive<D>) -> Self::Handle;
    /// Convert a [bool tensor](Backend::BoolTensorPrimitive) to a [handle](FusionBackend::Handle).
    fn bool_tensor_handle<const D: usize>(tensor: Self::BoolTensorPrimitive<D>) -> Self::Handle;
}
