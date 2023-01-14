pub use rumqttc::*;

#[cfg(feature = "embedded-svc")]
pub use embedded_svc_impl::*;

#[cfg(feature = "embedded-svc")]
mod embedded_svc_impl {
    use core::future::Future;
    use core::marker::PhantomData;

    use embedded_svc::mqtt::client::asynch::{
        Client, Connection, Details, ErrorType, Event, Message, MessageId, Publish, QoS,
    };
    use embedded_svc::mqtt::client::MessageImpl;

    use log::trace;

    use rumqttc::{AsyncClient, ClientError, ConnectionError, EventLoop, PubAck, SubAck, UnsubAck};

    pub struct MqttClient(AsyncClient);

    impl MqttClient {
        pub const fn new(client: AsyncClient) -> Self {
            Self(client)
        }
    }

    impl ErrorType for MqttClient {
        type Error = ClientError;
    }

    impl Client for MqttClient {
        type SubscribeFuture<'a>
        = impl Future<Output = Result<MessageId, Self::Error>> + Send + 'a
        where Self: 'a;

        type UnsubscribeFuture<'a>
        = impl Future<Output = Result<MessageId, Self::Error>> + Send + 'a
        where Self: 'a;

        fn subscribe<'a>(&'a mut self, topic: &'a str, qos: QoS) -> Self::SubscribeFuture<'a> {
            async move {
                self.0.subscribe(topic, to_qos(qos)).await?;

                Ok(0)
            }
        }

        fn unsubscribe<'a>(&'a mut self, topic: &'a str) -> Self::UnsubscribeFuture<'a> {
            async move {
                self.0.unsubscribe(topic).await?;

                Ok(0)
            }
        }
    }

    impl Publish for MqttClient {
        type PublishFuture<'a>
        = impl Future<Output = Result<MessageId, Self::Error>> + Send + 'a
        where Self: 'a;

        fn publish<'a>(
            &'a mut self,
            topic: &'a str,
            qos: embedded_svc::mqtt::client::QoS,
            retain: bool,
            payload: &'a [u8],
        ) -> Self::PublishFuture<'a> {
            async move {
                self.0.publish(topic, to_qos(qos), retain, payload).await?;

                Ok(0)
            }
        }
    }

    pub struct MessageRef<'a>(&'a rumqttc::Publish);

    impl<'a> MessageRef<'a> {
        pub fn into_message_impl(&self) -> Option<MessageImpl> {
            Some(MessageImpl::new(self))
        }
    }

    impl<'a> Message for MessageRef<'a> {
        fn id(&self) -> MessageId {
            self.0.pkid as _
        }

        fn topic(&self) -> Option<&'_ str> {
            Some(&self.0.topic)
        }

        fn data(&self) -> &'_ [u8] {
            &self.0.payload
        }

        fn details(&self) -> &Details {
            &Details::Complete
        }
    }

    pub struct MqttConnection<F, M>(EventLoop, F, PhantomData<fn() -> M>);

    impl<F, M> MqttConnection<F, M> {
        pub const fn new(event_loop: EventLoop, message_converter: F) -> Self {
            Self(event_loop, message_converter, PhantomData)
        }
    }

    impl<F, M> ErrorType for MqttConnection<F, M> {
        type Error = ConnectionError;
    }

    impl<F, M> Connection for MqttConnection<F, M>
    where
        F: FnMut(&MessageRef) -> Option<M> + Send,
        M: Send,
    {
        type Message = M;

        type NextFuture<'a>
        = impl Future<Output = Option<Result<Event<Self::Message>, Self::Error>>> + Send + 'a
        where Self: 'a;

        fn next(&mut self) -> Self::NextFuture<'_> {
            async move {
                loop {
                    let event = self.0.poll().await;
                    trace!("Got event: {:?}", event);

                    match event {
                        Ok(event) => {
                            let event = match event {
                                rumqttc::Event::Incoming(incoming) => match incoming {
                                    rumqttc::Packet::Connect(_) => Some(Event::BeforeConnect),
                                    rumqttc::Packet::ConnAck(_) => Some(Event::Connected(true)),
                                    rumqttc::Packet::Disconnect => Some(Event::Disconnected),
                                    rumqttc::Packet::PubAck(PubAck { pkid, .. }) => {
                                        Some(Event::Published(pkid as _))
                                    }
                                    rumqttc::Packet::SubAck(SubAck { pkid, .. }) => {
                                        Some(Event::Subscribed(pkid as _))
                                    }
                                    rumqttc::Packet::UnsubAck(UnsubAck { pkid, .. }) => {
                                        Some(Event::Unsubscribed(pkid as _))
                                    }
                                    rumqttc::Packet::Publish(publish) => {
                                        (self.1)(&MessageRef(&publish)).map(Event::Received)
                                    }
                                    _ => None,
                                },
                                rumqttc::Event::Outgoing(_) => None,
                            };

                            if let Some(event) = event {
                                return Some(Ok(event));
                            }
                        }
                        Err(err) => return Some(Err(err)),
                    }
                }
            }
        }
    }

    fn to_qos(qos: QoS) -> rumqttc::QoS {
        match qos {
            QoS::AtMostOnce => rumqttc::QoS::AtMostOnce,
            QoS::AtLeastOnce => rumqttc::QoS::AtLeastOnce,
            QoS::ExactlyOnce => rumqttc::QoS::ExactlyOnce,
        }
    }
}
