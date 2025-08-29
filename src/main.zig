const std = @import("std");

const print = @import("utilities/print.zig");
const logger = @import("utilities/logger.zig");
const gist = @import("dns/gist.zig");
const get_ip = @import("dns/get_ip.zig");
const dns = @import("dns/dns.zig");
const Config = @import("config/config.zig").Config;

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const arena_allocator = arena.allocator();

    const config = try Config.initFromJsonFile(arena_allocator);

    logger.info("Gist URI: {s}", .{config.gist_uri});
    for (config.ddns_domains.items) |domain| {
        logger.info("DDNS Domain: {s}", .{domain});
    }

    // TODO: pass the config object to this function
    const ip = get_ip.getIp(arena_allocator) catch {
        logger.fatal("Failed to get IP address", .{});
        std.process.exit(1);
    };

    try print.out("{s}\n", .{ip});
}
