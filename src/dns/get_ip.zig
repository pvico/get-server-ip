const std = @import("std");
const gist = @import("gist.zig");
const logger = @import("../utilities/logger.zig");
const dns = @import("dns.zig");
const constants = @import("../constants.zig");

const domains = [_][]const u8{
    constants.DDNS_DOMAIN_1,
    constants.DDNS_DOMAIN_2,
    constants.DDNS_DOMAIN_3,
    constants.DDNS_DOMAIN_4,
};

// Needed to compare with IP of "" domain
const NULL_DOMAIN: []const u8 = "0.0.0.0" ++ .{0} ** 9;

const IpError = error{IpNotFound};

pub fn getIp(allocator: std.mem.Allocator) ![]u8 {
    const ip = try allocator.alloc(u8, 16);
    // ip is returned, it cannot be freed normally in this scope
    errdefer allocator.free(ip);
    @memset(ip, 0);

    var aa = std.heap.ArenaAllocator.init(allocator);
    defer aa.deinit();
    const arena_allocator = aa.allocator();

    var gist_ip_valid = false;
    const gist_ip = gist.getIp(arena_allocator);
    if (gist_ip) |_| {
        gist_ip_valid = true;
    } else |_| {
        logger.warn("Failed to get Github gist IP", .{});
    }

    var domain_ips = std.StringHashMap([]const u8).init(arena_allocator);

    for (domains) |domain| {
        const dns_ip_local = dns.getIp(arena_allocator, domain);
        if (dns_ip_local) |ip_local| {
            // Check if the DNS IP matches the Gist IP
            // If it is the case, we will copy the DNS IP to the output and return it
            domain_ips.put(domain, ip_local) catch |err| {
                logger.err("Failed to store domain IP {s}: {any}", .{ ip_local, err });
            };
            if (gist_ip_valid) {
                if (gist_ip) |gist_ip_local| {
                    if (std.mem.eql(u8, ip_local, gist_ip_local)) {
                        // logger.debug("Domain {s} matches gist IP", .{domain});
                        @memcpy(ip, ip_local);
                        return ip;
                    }
                } else |_| unreachable;
            }
        } else |_| {
            logger.warn("Failed to get DNS IP for domain {s}", .{domain});
        }
    }

    // Here, no match between gist ip and domain ips
    // We check if 2 or more domain ips match between themselves
    // If it is the case, return the first matching IP
    for (domains, 0..) |domain1, i_1| {
            const domain1_ip = domain_ips.get(domain1);
        for (domains, 0..) |domain2, i_2| {
            const domain2_ip = domain_ips.get(domain2);
            if (i_1 != i_2 and domain1_ip != null and domain2_ip != null) {
                if (!std.mem.eql(u8, domain1_ip.?, NULL_DOMAIN)) {
                    if (std.mem.eql(u8, domain1_ip.?, domain2_ip.?)) {
                        @memcpy(ip, domain1_ip.?);
                        return ip;
                    }
                }
            }
        }
    }

    return IpError.IpNotFound;
}
