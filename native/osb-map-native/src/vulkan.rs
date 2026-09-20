//! Audited unsafe boundary: owns a dedicated Vulkan device for MapLibre only.
//! No Bevy handles are borrowed. The render session MUST be dropped before this owner.
#![allow(unsafe_code)]
use ash::{vk, vk::Handle};
use maplibre_native_ffi::{NativePointer, VulkanContextDescriptor};
use std::ffi::CString;
pub(crate) struct Vulkan {
    entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    physical: vk::PhysicalDevice,
    queue: vk::Queue,
    family: u32,
}
impl Vulkan {
    pub fn new(software: bool) -> Result<Self, String> {
        // SAFETY: ash loads the platform Vulkan loader and retains its library handle.
        let entry = unsafe { ash::Entry::load() }.map_err(|e| e.to_string())?;
        let name = CString::new("OpenStreetBattle basemap").map_err(|e| e.to_string())?;
        let app = vk::ApplicationInfo::default()
            .application_name(&name)
            .api_version(vk::API_VERSION_1_1);
        let info = vk::InstanceCreateInfo::default().application_info(&app);
        // SAFETY: app and its C string outlive this create call.
        let instance = unsafe { entry.create_instance(&info, None) }.map_err(|e| e.to_string())?;
        // SAFETY: instance is live, and no resources from other instances are passed.
        let devices = match unsafe { instance.enumerate_physical_devices() } {
            Ok(d) => d,
            Err(e) => {
                unsafe { instance.destroy_instance(None) };
                return Err(e.to_string());
            }
        };
        let mut options = Vec::new();
        for physical in devices {
            // SAFETY: these handles were enumerated from this live instance.
            let properties = unsafe { instance.get_physical_device_properties(physical) };
            if software && properties.device_type != vk::PhysicalDeviceType::CPU {
                continue;
            }
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(physical) };
            if let Some((family, _)) = families.iter().enumerate().find(|(_, p)| {
                p.queue_count > 0 && p.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            }) {
                let score = match properties.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                    _ => 1,
                };
                options.push((score, physical, family as u32));
            }
        }
        options.sort_by_key(|o| std::cmp::Reverse(o.0));
        let Some((_, physical, family)) = options.first().copied() else {
            // SAFETY: instance owns no child resources yet.
            unsafe { instance.destroy_instance(None) };
            return Err("no suitable Vulkan graphics queue".into());
        };
        let priorities = [1.0];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priorities)];
        // SAFETY: physical belongs to this instance.
        let supported = unsafe { instance.get_physical_device_features(physical) };
        let features = vk::PhysicalDeviceFeatures {
            sampler_anisotropy: supported.sampler_anisotropy,
            wide_lines: supported.wide_lines,
            ..Default::default()
        };
        let info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_features(&features);
        // SAFETY: queue family exists and all pointer-backed create-info storage is alive.
        let device = match unsafe { instance.create_device(physical, &info, None) } {
            Ok(d) => d,
            Err(e) => {
                unsafe { instance.destroy_instance(None) };
                return Err(e.to_string());
            }
        };
        // SAFETY: device was created with queue zero in this family.
        let queue = unsafe { device.get_device_queue(family, 0) };
        Ok(Self {
            entry,
            instance,
            device,
            physical,
            queue,
            family,
        })
    }
    pub fn descriptor(&self) -> VulkanContextDescriptor {
        // SAFETY: all handles and procedure pointers remain live until this owner is dropped.
        // NativeMap field/drop ordering destroys the session, map and runtime first.
        unsafe {
            let mut d = VulkanContextDescriptor::new(
                NativePointer::from_address(self.instance.handle().as_raw() as usize),
                NativePointer::from_address(self.physical.as_raw() as usize),
                NativePointer::from_address(self.device.handle().as_raw() as usize),
                NativePointer::from_address(self.queue.as_raw() as usize),
                self.family,
            );
            d.get_instance_proc_addr = NativePointer::from_address(
                self.entry.static_fn().get_instance_proc_addr as *const () as usize,
            );
            d.get_device_proc_addr = NativePointer::from_address(
                self.instance.fp_v1_0().get_device_proc_addr as *const () as usize,
            );
            d
        }
    }
}
impl Drop for Vulkan {
    fn drop(&mut self) {
        // SAFETY: no render session remains; the queue is idle before dependent handles are destroyed.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
