const std = @import("std");

const print = @import("utilities/print.zig");
const logger = @import("utilities/logger.zig");
const gist = @import("dns/gist.zig");
const get_ip = @import("dns/get_ip.zig");
const dns = @import("dns/dns.zig");
const Config = @import("config/config.zig").Config;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // config
    const exe_path = std.fs.selfExeDirPathAlloc(allocator) catch {
        logger.fatal("Failed to get executable path", .{});
        std.process.exit(1);
    };
    defer allocator.free(exe_path);

    const config_path_parts = [_][]const u8{
        exe_path,
        ".get-server-ip",
        "config.json",
    };
    const config_path = std.fs.path.join(allocator, &config_path_parts) catch {
        logger.fatal("Failed to create config path", .{});
        std.process.exit(1);
    };
    defer allocator.free(config_path);

    const config_file = std.fs.openFileAbsolute(config_path, .{}) catch {
        logger.fatal("Failed to open config file {s}", .{config_path});
        std.process.exit(1);
    };
    defer config_file.close();

    const json_config = try config_file.readToEndAlloc(allocator, std.math.maxInt(usize));
    defer allocator.free(json_config);

    var parsed_json_config = try Config.fromJson(allocator, json_config);
    defer parsed_json_config.deinit();

    // std.debug.print("Parsed JSON config: {any}", .{parsed_json_config});
    // const parse_json_config = &parsed_json_config.value;

    const ip = get_ip.getIp(allocator) catch {
        logger.fatal("Failed to get IP address", .{});
        std.process.exit(1);
    };
    defer allocator.free(ip);

    try print.out("{s}\n", .{ip});
}
