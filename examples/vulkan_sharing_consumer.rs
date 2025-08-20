// Vulkan Sharing Consumer Example
// 
// This example demonstrates how to receive shared Vulkan textures from a Bevy producer.
// In a real-world scenario, this would be a separate application (Qt, GTK, Unity, etc.)
// that consumes the shared textures for display or further processing.
//
// Key concepts demonstrated:
// 1. IPC connection via Unix socket
// 2. Receiving shared memory file descriptors
// 3. Frame synchronization using semaphores
// 4. Proper error handling and resource management
//
// To test this example:
// 1. Run the vulkan_sharing_producer example first
// 2. Run this consumer example in a separate terminal
// 3. The consumer will connect and receive frames from the producer

use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use nix::sys::socket::{self, ControlMessageOwned, MsgFlags};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IPCMetadata {
    width: u32,
    height: u32,
    format: u32,
    memory_fds: Vec<RawFd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IPCFrameInfo {
    buffer_index: usize,
    render_finished_semaphore_fd: Option<RawFd>,
    consumer_ready_semaphore_fd: Option<RawFd>,
}

struct ConsumerStats {
    frames_received: u64,
    total_processing_time: Duration,
    last_stats_print: Instant,
}

impl ConsumerStats {
    fn new() -> Self {
        Self {
            frames_received: 0,
            total_processing_time: Duration::ZERO,
            last_stats_print: Instant::now(),
        }
    }
    
    fn record_frame(&mut self, processing_time: Duration) {
        self.frames_received += 1;
        self.total_processing_time += processing_time;
        
        // Print stats every 2 seconds
        if self.last_stats_print.elapsed() >= Duration::from_secs(2) {
            self.print_stats();
            self.last_stats_print = Instant::now();
        }
    }
    
    fn print_stats(&self) {
        let avg_processing_time = if self.frames_received > 0 {
            self.total_processing_time.as_micros() as f64 / self.frames_received as f64
        } else {
            0.0
        };
        
        println!("📊 Consumer Stats:");
        println!("   Frames received: {}", self.frames_received);
        println!("   Avg processing time: {:.2}μs", avg_processing_time);
        println!("   Est. consumer FPS: {:.1}", 1_000_000.0 / avg_processing_time.max(1.0));
    }
}

fn main() {
    println!("🚀 Vulkan Sharing Consumer Starting...");
    println!("📺 This example receives shared Vulkan textures from a Bevy producer");
    println!("🔗 Connecting to producer via Unix socket...");
    
    let socket_path = "/tmp/bevy_vulkan_sharing.sock";
    let mut stats = ConsumerStats::new();
    
    // Attempt connection with retry logic
    let mut stream = match connect_with_retry(socket_path, 5) {
        Some(stream) => {
            println!("✅ Connected to producer at {}", socket_path);
            stream
        }
        None => {
            eprintln!("❌ Failed to connect to producer after multiple attempts");
            eprintln!("💡 Make sure the vulkan_sharing_producer example is running first");
            return;
        }
    };
    
    // Receive initial metadata with shared memory handles
    match receive_metadata(&mut stream) {
        Ok(metadata) => {
            println!("📋 Received shared surface metadata:");
            println!("   📐 Resolution: {}x{}", metadata.width, metadata.height);
            println!("   🎨 Vulkan Format: {} ({})", metadata.format, format_name(metadata.format));
            println!("   💾 Shared Memory FDs: {} buffers", metadata.memory_fds.len());
            
            // Print implementation guidance
            print_vulkan_integration_guide(&metadata);
            
            println!("🎬 Starting frame processing loop...");
            println!("   Press Ctrl+C to exit");
            
            // Main frame processing loop
            loop {
                let frame_start = Instant::now();
                
                match receive_frame_info(&mut stream) {
                    Ok(frame_info) => {
                        process_frame(&frame_info, &metadata);
                        
                        // Record processing stats
                        stats.record_frame(frame_start.elapsed());
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to receive frame info: {}", e);
                        eprintln!("🔄 Producer may have disconnected");
                        break;
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to receive metadata: {}", e);
            eprintln!("🔧 Check if producer is configured correctly");
        }
    }
    
    println!("🏁 Consumer shutting down");
}

fn connect_with_retry(socket_path: &str, max_retries: u32) -> Option<UnixStream> {
    for attempt in 1..=max_retries {
        match UnixStream::connect(socket_path) {
            Ok(stream) => return Some(stream),
            Err(e) => {
                if attempt < max_retries {
                    println!("🔄 Connection attempt {} failed: {}", attempt, e);
                    println!("   Retrying in 1 second...");
                    std::thread::sleep(Duration::from_secs(1));
                } else {
                    eprintln!("❌ Final connection attempt failed: {}", e);
                }
            }
        }
    }
    None
}

fn process_frame(frame_info: &IPCFrameInfo, _metadata: &IPCMetadata) {
    println!("🎞️  Frame received - Buffer index: {}", frame_info.buffer_index);
    
    // In a real Vulkan consumer application, you would:
    // 
    // 1. WAIT FOR PRODUCER: Import and wait on render_finished_semaphore_fd
    //    - vkImportSemaphoreFdKHR() to import the semaphore
    //    - vkWaitSemaphores() or use it in vkQueueSubmit() wait stage
    //
    // 2. USE SHARED TEXTURE: Access the shared memory
    //    - The shared VkDeviceMemory is already bound to a VkImage
    //    - Create VkImageView if needed for your pipeline
    //    - Use in compute shader, graphics pipeline, or copy operations
    //
    // 3. SIGNAL COMPLETION: Signal consumer_ready_semaphore_fd when done
    //    - Import consumer_ready_semaphore_fd if provided
    //    - vkQueueSubmit() with signal semaphore, or vkSignalSemaphore()
    //
    // 4. SYNCHRONIZATION: This ensures proper frame pacing and prevents tearing
    
    // Simulate realistic processing time (rendering/copying/displaying)
    let processing_time = Duration::from_millis(8); // Simulate ~8ms processing
    std::thread::sleep(processing_time);
    
    // In real implementation, you'd handle semaphore FDs here
    if frame_info.render_finished_semaphore_fd.is_some() {
        // Would wait on this semaphore before using texture
        println!("   🚦 Render finished semaphore available");
    }
    
    if frame_info.consumer_ready_semaphore_fd.is_some() {
        // Would signal this semaphore after processing
        println!("   ✅ Consumer ready semaphore available");
    }
}

fn format_name(vk_format: u32) -> &'static str {
    // Convert Vulkan format enum to human-readable name
    match vk_format {
        37 => "B8G8R8A8_SRGB",
        44 => "B8G8R8A8_UNORM", 
        43 => "R8G8B8A8_SRGB",
        42 => "R8G8B8A8_UNORM", // Fixed duplicate case
        _ => "Unknown",
    }
}

fn print_vulkan_integration_guide(metadata: &IPCMetadata) {
    println!("\n🔧 Integration Guide for Real Vulkan Applications:");
    println!("   1. Import Memory FDs:");
    println!("      - Use vkImportMemoryFdKHR() for each memory_fd");
    println!("      - Create VkImage with matching dimensions ({}x{})", metadata.width, metadata.height);
    println!("      - Bind imported memory with vkBindImageMemory()");
    println!();
    println!("   2. Create Image Views:");
    println!("      - vkCreateImageView() for each imported image");
    println!("      - Use format {} (Vulkan enum {})", format_name(metadata.format), metadata.format);
    println!();
    println!("   3. Synchronization:");
    println!("      - Import semaphore FDs with vkImportSemaphoreFdKHR()");
    println!("      - Wait on render_finished before using texture");
    println!("      - Signal consumer_ready after processing");
    println!();
    println!("   4. Usage Examples:");
    println!("      - Texture binding: Use in fragment shaders");
    println!("      - Compute processing: Dispatch compute shaders");
    println!("      - Display: Present to swapchain or copy to window");
    println!();
}

fn receive_metadata(stream: &mut UnixStream) -> Result<IPCMetadata, Box<dyn std::error::Error>> {
    println!("📥 Receiving metadata from producer...");
    
    let mut buf = vec![0u8; 1024];
    let mut cmsg_buf = nix::cmsg_space!([RawFd; 10]);
    
    let fd = stream.as_raw_fd();
    
    let bytes_received;
    let mut received_fds = Vec::new();
    
    {
        let mut iov = [std::io::IoSliceMut::new(&mut buf)];
        let msg = socket::recvmsg::<()>(
            fd,
            &mut iov,
            Some(&mut cmsg_buf),
            MsgFlags::empty(),
        )?;
        
        bytes_received = msg.bytes;
        
        if bytes_received == 0 {
            return Err("Producer closed connection during metadata transfer".into());
        }
        
        // Extract file descriptors from control messages
        if let Ok(cmsgs) = msg.cmsgs() {
            for cmsg in cmsgs {
                if let ControlMessageOwned::ScmRights(fds) = cmsg {
                    received_fds.extend_from_slice(&fds);
                    println!("   📎 Received {} memory file descriptors", fds.len());
                }
            }
        }
    }
    
    // Deserialize metadata
    let mut metadata: IPCMetadata = bincode::deserialize(&buf[..bytes_received])
        .map_err(|e| format!("Failed to deserialize metadata: {}", e))?;
    
    metadata.memory_fds = received_fds;
    
    // Validate metadata
    if metadata.width == 0 || metadata.height == 0 {
        return Err("Invalid texture dimensions received".into());
    }
    
    if metadata.memory_fds.is_empty() {
        return Err("No memory file descriptors received".into());
    }
    
    println!("   ✅ Metadata received and validated");
    Ok(metadata)
}

fn receive_frame_info(stream: &mut UnixStream) -> Result<IPCFrameInfo, Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 256];
    let mut cmsg_buf = nix::cmsg_space!([RawFd; 2]);
    
    let fd = stream.as_raw_fd();
    
    let bytes_received;
    let mut received_fds = Vec::new();
    
    {
        let mut iov = [std::io::IoSliceMut::new(&mut buf)];
        let msg = socket::recvmsg::<()>(
            fd,
            &mut iov,
            Some(&mut cmsg_buf),
            MsgFlags::empty(),
        )?;
        
        bytes_received = msg.bytes;
        
        if bytes_received == 0 {
            return Err("Producer closed connection".into());
        }
        
        // Extract semaphore FDs if present
        if let Ok(cmsgs) = msg.cmsgs() {
            for cmsg in cmsgs {
                if let ControlMessageOwned::ScmRights(fds) = cmsg {
                    received_fds.extend_from_slice(&fds);
                }
            }
        }
    }
    
    // Deserialize frame info
    let mut frame_info: IPCFrameInfo = bincode::deserialize(&buf[..bytes_received])
        .map_err(|e| format!("Failed to deserialize frame info: {}", e))?;
    
    // Assign received FDs to semaphores
    if !received_fds.is_empty() {
        if received_fds.len() >= 1 {
            frame_info.render_finished_semaphore_fd = Some(received_fds[0]);
        }
        if received_fds.len() >= 2 {
            frame_info.consumer_ready_semaphore_fd = Some(received_fds[1]);
        }
    }
    
    Ok(frame_info)
}