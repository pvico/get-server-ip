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

const AddressStatus = struct {
    dynamic: bool = false,
    deprecated: bool = false,

    fn rank(self: AddressStatus) u2 {
        var value: u2 = 0;
        if (self.dynamic) value |= 1;
        if (self.deprecated) value |= 2;
        return value;
    }

    fn format(self: AddressStatus, buffer: []u8) []const u8 {
        var len: usize = 0;
        if (!self.dynamic and !self.deprecated) {
            const word = "stable";
            std.mem.copyForwards(u8, buffer[0..word.len], word);
            len = word.len;
        } else {
            if (self.dynamic) {
                const word = "dynamic";
                std.mem.copyForwards(u8, buffer[0..word.len], word);
                len = word.len;
            }
            if (self.deprecated) {
                if (len != 0) {
                    buffer[len] = '+';
                    len += 1;
                }
                const word = "deprecated";
                std.mem.copyForwards(u8, buffer[len .. len + word.len], word);
                len += word.len;
            }
        }
        return buffer[0..len];
    }
};

fn addressStatusFromFlags(flags: u32) AddressStatus {
    return .{
        .dynamic = (flags & 0x80) == 0,
        .deprecated = (flags & 0x20) != 0,
    };
}

const AddressStatusMap = std.AutoHashMap([16]u8, AddressStatus);

pub fn debugPrintLinuxInterfaces(allocator: std.mem.Allocator) !void {
    if (builtin.target.os.tag != .linux) return;

    const file_data = std.fs.cwd().readFileAlloc(allocator, "/proc/net/if_inet6", 64 * 1024) catch |err| {
        logger.warn("Unable to read /proc/net/if_inet6: {s}", .{@errorName(err)});
        return;
    };
    defer allocator.free(file_data);

    var lines = std.mem.splitScalar(u8, file_data, '\n');
    while (lines.next()) |line| {
        if (line.len == 0) continue;

        var tokens = std.mem.tokenizeAny(u8, line, " \t");
        _ = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        const flags_token = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;

        const flags = std.fmt.parseInt(u32, flags_token, 16) catch continue;
        const status = addressStatusFromFlags(flags);
        var status_buf: [32]u8 = undefined;
        const status_slice = status.format(status_buf[0..]);

        try print.err("{s} {s}\n", .{ line, status_slice });
    }
}

fn interfaceExists(list: ?*c.struct_ifaddrs, name: [*:0]const u8) bool {
    var cursor = list;
    while (cursor) |node| {
        if (c.strcmp(node.*.ifa_name, name) == 0) {
            return true;
        }
        cursor = node.*.ifa_next;
    }
    return false;
}

fn linuxLoadAddressStatus(
    allocator: std.mem.Allocator,
    interface_name: [*:0]const u8,
) AddressStatusMap {
    var map = AddressStatusMap.init(allocator);

    if (builtin.target.os.tag != .linux) return map;

    const file_data = std.fs.cwd().readFileAlloc(allocator, "/proc/net/if_inet6", 64 * 1024) catch |err| {
        logger.warn("Unable to read /proc/net/if_inet6: {s}", .{@errorName(err)});
        return map;
    };
    defer allocator.free(file_data);

    const iface = std.mem.span(interface_name);

    var lines = std.mem.splitScalar(u8, file_data, '\n');
    while (lines.next()) |line| {
        if (line.len == 0) continue;

        var tokens = std.mem.tokenizeAny(u8, line, " \t");
        const addr_token = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        _ = tokens.next() orelse continue;
        const flags_token = tokens.next() orelse continue;
        const name_token = tokens.next() orelse continue;

        if (!std.mem.eql(u8, name_token, iface)) continue;
        if (addr_token.len != 32) continue;

        var addr_bytes: [16]u8 = undefined;
        var valid = true;
        var i: usize = 0;
        while (i < addr_token.len) : (i += 2) {
            const value = std.fmt.parseInt(u8, addr_token[i .. i + 2], 16) catch {
                valid = false;
                break;
            };
            addr_bytes[i / 2] = value;
        }
        if (!valid) continue;

        const flags = std.fmt.parseInt(u32, flags_token, 16) catch continue;
        const status = addressStatusFromFlags(flags);

        map.put(addr_bytes, status) catch {
            // Ignore insertion failures; continue best effort.
        };
    }

    return map;
}

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
    var interface_name: [*:0]const u8 = switch (builtin.target.os.tag) {
        .macos => "en0",
        .linux => "eth0",
        else => "eth0",
    };

    var gpa: ?*c.struct_ifaddrs = null;
    if (c.getifaddrs(&gpa) != 0) {
        return error.GetIfAddrsFailed;
    }
    defer c.freeifaddrs(gpa.?);

    if (builtin.target.os.tag == .linux) {
        if (!interfaceExists(gpa, "eth0")) {
            if (interfaceExists(gpa, "wlan0")) {
                interface_name = "wlan0";
            }
        }
    }

    var linux_status_map: ?AddressStatusMap = null;
    if (builtin.target.os.tag == .linux) {
        linux_status_map = linuxLoadAddressStatus(allocator, interface_name);
    }
    defer if (linux_status_map) |*map| map.deinit();

    var p = gpa;
    var best_buf: [c.INET6_ADDRSTRLEN]u8 = undefined;
    var best_len: usize = 0;
    var best_scope: AddressScope = .other;
    var best_prefix: u8 = 255;
    var best_status: AddressStatus = .{};
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
                var status = AddressStatus{};
                if (linux_status_map) |*map| {
                    const addr_bytes = in6AddrBytes(&sa6.*.sin6_addr).*;
                    if (map.get(addr_bytes)) |entry| {
                        status = entry;
                    }
                }

                const prefix_len = getPrefixLength(node.*.ifa_netmask);

                if (!have_best or shouldPrefer(
                    scope,
                    prefix_len,
                    addr_slice,
                    best_scope,
                    best_prefix,
                    best_buf[0..best_len],
                    status,
                    best_status,
                )) {
                    std.mem.copyForwards(u8, best_buf[0..nul_idx], addr_slice);
                    best_len = nul_idx;
                    best_scope = scope;
                    best_prefix = prefix_len;
                    best_status = status;
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
    new_status: AddressStatus,
    current_status: AddressStatus,
) bool {
    const new_rank = new_status.rank();
    const current_rank = current_status.rank();

    if (new_rank != current_rank) {
        return new_rank < current_rank;
    }

    const new_score: u3 = @intFromEnum(new_scope);
    const current_score: u3 = @intFromEnum(current_scope);

    if (new_score != current_score) {
        const new_is_better = new_score > current_score;
        // if (new_is_better) {
        //     print.err("Discarding address {s}\n", .{current_addr}) catch {};
        // } else {
        //     print.err("Discarding address {s}\n", .{new_addr}) catch {};
        // }
        return new_is_better;
    }

    if (new_prefix != current_prefix) {
        const new_is_better = new_prefix > current_prefix;
        // if (new_is_better) {
        //     print.err("Discarding address {s}\n", .{current_addr}) catch {};
        // } else {
        //     print.err("Discarding address {s}\n", .{new_addr}) catch {};
        // }
        return new_is_better;
    }

    return std.mem.lessThan(u8, new_addr, current_addr);
}
