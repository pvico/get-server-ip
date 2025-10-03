const std = @import("std");
const builtin = @import("builtin");

const print = @import("utilities/print.zig");
const logger = @import("utilities/logger.zig");

const local_ethernet = @import("network/local_ethernet.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    // var gpa = std.heap.DebugAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // const config = try Config.initFromJsonFile(arena_allocator);

    // const ip = get_ip.getIp(arena_allocator, config) catch {
    //     logger.fatal("Failed to get IP address", .{});
    //     std.process.exit(1);
    // };

    const ip = local_ethernet.getLocalIPv6(allocator) catch {
        logger.fatal("Failed to get IP address", .{});
        std.process.exit(1);
    };
    defer allocator.free(ip);

    if (builtin.os.tag == .linux) {
        try local_ethernet.debugPrintLinuxInterfaces(allocator);
    }

    try print.out("{s}\n", .{ip});
}
