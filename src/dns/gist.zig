const std = @import("std");
const constants = @import("../constants.zig");

pub fn getIp(allocator: std.mem.Allocator) ![]u8 {
    const ip = try allocator.alloc(u8, 16);
    // ip is returned, it cannot be freed normally in this scope
    errdefer allocator.free(ip);
    @memset(ip, 0);
    var writer: std.Io.Writer = .fixed(ip);

    const uri: std.Uri = try std.Uri.parse(constants.GIST_URI);

    var client = std.http.Client{ .allocator = allocator };
    defer client.deinit();

    const headers = &[_]std.http.Header{
        .{ .name = "User-Agent", .value = "zig-http-client" },
    };

    const fetchResult = try client.fetch(.{
        .method = .GET,
        .location = .{ .uri = uri },
        .extra_headers = headers,
        .response_writer = &writer,
    });

    if (fetchResult.status != .ok) {
        return error.HttpError;
    }

    return ip;
}
