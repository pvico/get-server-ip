const std = @import("std");
const json = std.json;
const logger = @import("../utilities/logger.zig");

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
        if (json_value_source != .object) {
            return error.UnexpectedToken;
        }

        var result_config = try Self.init(allocator);
        errdefer result_config.deinit(allocator);

        var json_value_source_iterator = json_value_source.object.iterator();

        while (json_value_source_iterator.next()) |json_key_value| {
            inline for (std.meta.fields(Self)) |field_info| {
                const struct_field_name = field_info.name;
                if (std.mem.eql(u8, struct_field_name, json_key_value.key_ptr.*)) {
                    if (std.mem.eql(u8, struct_field_name, "ddns_domains")) {
                        // struct_field_name equals "ddns_domains"
                        for (json_key_value.value_ptr.array.items) |item| {
                            const ddns_uri = item.string;
                            // logger.info("ddns_uri: {s}", .{ddns_uri});
                            try result_config.add_ddns_domain(allocator, ddns_uri);
                        }
                    } else if (std.mem.eql(u8, struct_field_name, "gist_uri")) {
                        // struct_field_name equals "gist_uri"
                        result_config.gist_uri = json_key_value.value_ptr.string;
                    } else {
                        return error.UnexpectedToken;
                    }
                }
            }
        }

        return result_config;
    }

    pub fn fromJson(allocator: std.mem.Allocator, json_string: []const u8) !json.Parsed(Self) {
        const parsedValue = try json.parseFromSlice(json.Value, allocator, json_string, .{});
        defer parsedValue.deinit();
        
        return try json.parseFromValue(Self, allocator, parsedValue.value, .{});
    }
};
