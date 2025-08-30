const std = @import("std");
const json = std.json;
const logger = @import("../utilities/logger.zig");
const builtin = @import("builtin");

const DEBUG_MODE = builtin.mode == std.builtin.OptimizeMode.Debug;

pub const Config = struct {
    const Self = @This();
    gist_uri: []const u8,
    ddns_domains: std.ArrayList([]const u8),

    pub fn init(allocator: std.mem.Allocator) !Self {
        return .{
            .gist_uri = "",
            .ddns_domains = try std.ArrayList([]const u8).initCapacity(allocator, 6),
        };
    }

    pub fn deinit(self: *Self, allocator: std.mem.Allocator) void {
        self.ddns_domains.deinit(allocator);
        self.* = undefined;
    }

    pub fn clone(self: *const Self) !Self {
        return .{
            .gist_uri = self.gist_uri,
            .ddns_domains = try self.ddns_domains.clone(),
        };
    }

    pub fn add_ddns_domain(self: *Self, allocator: std.mem.Allocator, domain: []const u8) !void {
        try self.ddns_domains.append(allocator, domain);
    }

    pub fn jsonParseFromValue(allocator: std.mem.Allocator, json_value_source: std.json.Value, _: json.ParseOptions) !Self {
        if (json_value_source != .object) return error.UnexpectedToken;

        var config = try Self.init(allocator);
        errdefer config.deinit(allocator);

        var json_value_source_iterator = json_value_source.object.iterator();

        while (json_value_source_iterator.next()) |json_key_value| {
            const key = json_key_value.key_ptr.*;
            const val = json_key_value.value_ptr.*;
            if (std.mem.eql(u8, "ddns_domains", key)) {
                for (val.array.items) |item| {
                    if (item != .string) return error.UnexpectedToken;
                    try config.add_ddns_domain(allocator, item.string);
                }
            } else if (std.mem.eql(u8, "gist_uri", key)) {
                if (val != .string) return error.UnexpectedToken;
                config.gist_uri = val.string;
            } else {
                return error.UnexpectedToken;
            }
        }

        return config;
    }

    pub fn fromJson(allocator: std.mem.Allocator, json_string: []const u8) !Self {
        const parsedValue = try json.parseFromSlice(json.Value, allocator, json_string, .{});

        const parsed_config = try json.parseFromValue(Self, allocator, parsedValue.value, .{});

        return parsed_config.value;
    }

    pub fn initFromJsonFile(arena_allocator: std.mem.Allocator) !Self {
        const exe_path = std.fs.selfExeDirPathAlloc(arena_allocator) catch {
            logger.fatal("Failed to get executable path", .{});
            std.process.exit(1);
        };

        const config_path_parts = [_][]const u8{
            exe_path,
            ".get-server-ip",
            "config.json",
        };
        const config_path = std.fs.path.join(arena_allocator, &config_path_parts) catch {
            logger.fatal("Failed to create config path", .{});
            std.process.exit(1);
        };

        const config_file = std.fs.openFileAbsolute(config_path, .{}) catch {
            logger.fatal("Failed to open config file {s}", .{config_path});
            std.process.exit(1);
        };
        defer config_file.close();

        const json_config = try config_file.readToEndAlloc(arena_allocator, std.math.maxInt(usize));
        const config = try Config.fromJson(arena_allocator, json_config);

        if (DEBUG_MODE) {
            logger.debug("Gist URI: {s}", .{config.gist_uri});
            for (config.ddns_domains.items) |domain| {
                logger.debug("DDNS Domain: {s}", .{domain});
            }
        }

        return config;
    }
};
