const std = @import("std");
const builtin = @import("builtin");
const logger = @import("../utilities/logger.zig");
const print = @import("../utilities/print.zig");

const c = @cImport({
    @cInclude("ifaddrs.h");
    @cInclude("arpa/inet.h");
    @cInclude("net/if.h");
    @cInclude("string.h");
});

fn in6AddrBytes(addr: *align(1) const c.struct_in6_addr) *const [16]u8 {
    if (@hasField(c.struct_in6_addr, "__u6_addr")) {
        return &addr.*.__u6_addr.__u6_addr8;
    } else if (@hasField(c.struct_in6_addr, "__in6_u")) {
        return &addr.*.__in6_u.__u6_addr8;
    } else {
        @compileError("Unsupported struct_in6_addr layout");
    }
}

const AddressScope = enum(u3) {
    other = 0,
    link_local = 1,
    unique_local = 2,
    global = 3,
};

fn classifyIPv6(addr: *align(1) const c.struct_in6_addr) ?AddressScope {
    const bytes = in6AddrBytes(addr);
    const slice = bytes[0..];

    if (std.mem.allEqual(u8, slice, 0)) {
        return null;
    }

    var loopback = true;
    for (slice[0..15]) |b| {
        if (b != 0) {
            loopback = false;
            break;
        }
    }
    if (loopback and slice[15] == 1) {
        return null;
    }

    if (slice[0] == 0xff) {
        return null;
    }

    if (slice[0] == 0xfe and (slice[1] & 0xc0) == 0x80) {
        return .link_local;
    }

    if ((slice[0] & 0xfe) == 0xfc) {
        return .unique_local;
    }

    if ((slice[0] & 0xe0) == 0x20) {
        return .global;
    }

    return .other;
}

fn ipv6PrefixLength(addr: *align(1) const c.struct_in6_addr) u8 {
    const bytes = in6AddrBytes(addr);
    const slice = bytes[0..];
    var total: u8 = 0;
    for (slice) |b| {
        if (b == 0xff) {
            total += 8;
            continue;
        }

        var value = b;
        while (value & 0x80 != 0) : (value <<= 1) {
            total += 1;
        }
        break;
    }
    return total;
}

fn getPrefixLength(mask: ?*c.struct_sockaddr) u8 {
    const netmask = mask orelse return 128;
    if (netmask.*.sa_family != c.AF_INET6) {
        return 128;
    }

    const sa6: *align(1) const c.struct_sockaddr_in6 = @as(
        *align(1) const c.struct_sockaddr_in6,
        @ptrCast(netmask),
    );

    return ipv6PrefixLength(&sa6.*.sin6_addr);
}

pub fn getLocalIPv6(allocator: std.mem.Allocator) ![]u8 {
    const interface_name: [*:0]const u8 = switch (builtin.target.os.tag) {
        .macos => "en0",
        .linux => "eth0",
        else => "eth0",
    };

    var gpa: ?*c.struct_ifaddrs = null;
    if (c.getifaddrs(&gpa) != 0) {
        return error.GetIfAddrsFailed;
    }
    defer c.freeifaddrs(gpa.?);

    var p = gpa;
    var best_buf: [c.INET6_ADDRSTRLEN]u8 = undefined;
    var best_len: usize = 0;
    var best_scope: AddressScope = .other;
    var best_prefix: u8 = 255;
    var have_best = false;
    while (p) |node| {
        if (c.strcmp(node.*.ifa_name, interface_name) == 0 and node.*.ifa_addr != null) {
            if (node.*.ifa_addr.*.sa_family == c.AF_INET6) {
                const sa6: *align(1) const c.struct_sockaddr_in6 = @as(
                    *align(1) const c.struct_sockaddr_in6,
                    @ptrCast(node.*.ifa_addr),
                );
                var buf: [c.INET6_ADDRSTRLEN]u8 = undefined;
                if (c.inet_ntop(
                    c.AF_INET6,
                    @ptrCast(&sa6.*.sin6_addr),
                    &buf[0],
                    buf.len,
                ) == null) {
                    return error.FormatIPv6Failed;
                }

                const nul_idx = std.mem.indexOfScalar(u8, buf[0..], 0) orelse buf.len;
                const addr_slice = buf[0..nul_idx];
                const scope = classifyIPv6(&sa6.*.sin6_addr) orelse {
                    continue;
                };
                const prefix_len = getPrefixLength(node.*.ifa_netmask);

                if (!have_best or shouldPrefer(scope, prefix_len, addr_slice, best_scope, best_prefix, best_buf[0..best_len])) {
                    std.mem.copyForwards(u8, best_buf[0..nul_idx], addr_slice);
                    best_len = nul_idx;
                    best_scope = scope;
                    best_prefix = prefix_len;
                    have_best = true;
                }
            }
        }
        p = node.*.ifa_next;
    }

    if (have_best) {
        return try allocator.dupe(u8, best_buf[0..best_len]);
    }

    return error.IPv6NotFound;
}

fn shouldPrefer(
    new_scope: AddressScope,
    new_prefix: u8,
    new_addr: []const u8,
    current_scope: AddressScope,
    current_prefix: u8,
    current_addr: []const u8,
) bool {
    const new_score: u3 = @intFromEnum(new_scope);
    const current_score: u3 = @intFromEnum(current_scope);

    if (new_score != current_score) {
        return new_score > current_score;
    }

    if (new_prefix != current_prefix) {
        return new_prefix < current_prefix;
    }

    return std.mem.lessThan(u8, new_addr, current_addr);
}
