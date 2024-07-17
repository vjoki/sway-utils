#![forbid(unsafe_code)]
// Rust port of https://github.com/alebastr/sway-systemd/blob/main/src/locale1-xkb-config
use argh::FromArgs;
use anyhow::Context;
use tracing::{debug, error, info};
use zbus::fdo;
use zbus_macros::proxy;

#[proxy(
    interface = "org.freedesktop.locale1",
    default_path = "/org/freedesktop/locale1",
    blocking_name = "Locale1Blocking",
)]
trait Locale1 {
    #[zbus(property)]
    fn x11_layout(&self) -> fdo::Result<String>;

    #[zbus(property)]
    fn x11_model(&self) -> fdo::Result<String>;

    #[zbus(property)]
    fn x11_variant(&self) -> fdo::Result<String>;

    #[zbus(property)]
    fn x11_options(&self) -> fdo::Result<String>;
}

struct XkbProperties {
    layout: String,
    model: String,
    variant: String,
    options: String,
}

struct DbusLocale1(zbus::blocking::Connection);

impl DbusLocale1 {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self(zbus::blocking::Connection::system()?))
    }

    pub fn properties_changed_stream(&self) -> anyhow::Result<zbus::blocking::fdo::PropertiesChangedIterator> {
        let proxy = zbus::blocking::fdo::PropertiesProxy::builder(&self.0)
            .destination("org.freedesktop.locale1")?
            .path("/org/freedesktop/locale1")?
            .build()?;
        Ok(proxy.receive_properties_changed()?)
    }

    pub fn get_xkb_properties(&self) -> anyhow::Result<XkbProperties> {
        let proxy = Locale1Blocking::builder(&self.0)
            .destination("org.freedesktop.locale1")?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()?;

        Ok(XkbProperties {
            layout: proxy.x11_layout()?,
            model: proxy.x11_model()?,
            variant: proxy.x11_variant()?,
            options: proxy.x11_options()?,
        })
    }
}

enum XkbProperty {
    Layout,
    Model,
    Variant,
    Options
}

impl AsRef<str> for XkbProperty {
    fn as_ref(&self) -> &str {
        match self {
            XkbProperty::Layout => "xkb_layout",
            XkbProperty::Model => "xkb_model",
            XkbProperty::Variant => "xkb_variant",
            XkbProperty::Options => "xkb_options",
        }
    }
}

struct SwayIpc(swayipc::Connection);

impl SwayIpc {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self(swayipc::Connection::new()?))
    }

    pub fn set_xkb_property(&mut self, device: &str, prop: XkbProperty, value: &str) {
        let cmd = format!("input {} {} '{}'", device, prop.as_ref(), value);
        if let Err(e) = self.0.run_command(&cmd) {
            error!(error = ?e, command = cmd, "Sway command failed");
        }
    }
}

/// Sync Sway input configuration with org.freedesktop.locale1.
#[derive(FromArgs)]
struct Args {
    /// control settings for a specific device identifier (see man sway-input; default: type:keyboard)
    #[argh(option, default="\"type:keyboard\".to_string()")]
    device: String,
    /// set logging level (default: info)
    #[argh(option, default="tracing::Level::INFO")]
    log_level: tracing::Level,
    /// apply current settings and exit immediately
    #[argh(switch)]
    oneshot: bool,
}

fn main() -> anyhow::Result<()> {
    let args: Args = argh::from_env();

    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .compact()
        .init();

    let dbus = DbusLocale1::new().context("D-Bus")?;
    let props = dbus.get_xkb_properties().context("D-Bus")?;

    let mut sway = SwayIpc::new().context("Sway IPC")?;
    info!("xkb({}): layout '{}' model '{}' variant '{}' options '{}'",
          args.device, props.layout, props.model, props.variant, props.options);
    sway.set_xkb_property(&args.device, XkbProperty::Layout, &props.layout);
    sway.set_xkb_property(&args.device, XkbProperty::Model, &props.model);
    sway.set_xkb_property(&args.device, XkbProperty::Variant, &props.variant);
    sway.set_xkb_property(&args.device, XkbProperty::Options, &props.options);

    if !args.oneshot {
        while let Some(signal) = dbus.properties_changed_stream()?.next() {
            let signal = signal.args()?;
            if signal.interface_name() == "org.freedesktop.locale1" {
                for (name, value) in signal.changed_properties().iter() {
                    let value: String = value.try_into()?;
                    info!("xkb({}): {} '{}'", args.device, name, value);
                    match *name {
                        "X11Layout"  => sway.set_xkb_property(&args.device, XkbProperty::Layout, &value),
                        "X11Model"   => sway.set_xkb_property(&args.device, XkbProperty::Model, &value),
                        "X11Variant" => sway.set_xkb_property(&args.device, XkbProperty::Variant, &value),
                        "X11Options" => sway.set_xkb_property(&args.device, XkbProperty::Options, &value),
                        _ => debug!(name, value=?value, "unhandled property"),
                    }
                }
            }
        }
    }

    Ok(())
}
