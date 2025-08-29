const std = @import("std");

const print = @import("utilities/print.zig");
const logger = @import("utilities/logger.zig");
const gist = @import("dns/gist.zig");
const get_ip = @import("dns/get_ip.zig");
const dns = @import("dns/dns.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const ip = get_ip.getIp(allocator) catch {
        logger.fatal("Failed to get IP address", .{});
        std.process.exit(1);
    };
    defer allocator.free(ip);

    try print.out("{s}\n", .{ip});
}
