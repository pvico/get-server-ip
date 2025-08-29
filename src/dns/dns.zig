const std = @import("std");
const logger = @import("../utilities/logger.zig");
const print = @import("../utilities/print.zig");

pub fn getIp(allocator: std.mem.Allocator, domain: []const u8) ![]u8 {

    const ip: []u8 = try allocator.alloc(u8, 16);
    // ip is returned, it cannot be freed normally in this scope
    errdefer allocator.free(ip);
    @memset(ip, 0);

    const addr_list = try std.net.getAddressList(allocator, domain, 80);
    defer addr_list.deinit();

    if (addr_list.addrs.len == 0) {
        return error.NoAddressFound;
    }

    const ipv4_address =  addr_list.addrs[0].in;
    const bytes: *const [4]u8 = @ptrCast(&ipv4_address.sa.addr);
    _ = try std.fmt.bufPrint(ip, "{d}.{d}.{d}.{d}", .{bytes[0], bytes[1], bytes[2], bytes[3]});

    return ip;
}
