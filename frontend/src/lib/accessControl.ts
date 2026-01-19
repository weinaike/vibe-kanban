/**
 * Access control utilities for determining local vs remote access patterns
 */

/**
 * Check if the given hostname is a local/LAN address
 *
 * Local addresses: localhost, 127.0.0.1, 0.0.0.0, ::1
 * LAN addresses: 10.x.x.x, 172.16-31.x.x, 192.168.x.x, .local domains
 *
 * @param hostname - The hostname to check (defaults to current window.location.hostname)
 * @returns true if the hostname is local/LAN, false if it's a public domain
 */
export function isLocalOrLanAccess(hostname: string = window.location.hostname): boolean {
  // Localhost variants
  if (
    hostname === 'localhost' ||
    hostname === '127.0.0.1' ||
    hostname === '0.0.0.0' ||
    hostname === '::1' ||
    hostname === '[::1]'
  ) {
    return true;
  }

  // .local domains (mDNS/Bonjour)
  if (hostname.endsWith('.local')) {
    return true;
  }

  // Private IP ranges
  const parts = hostname.split('.');
  if (parts.length === 4) {
    const firstOctet = parseInt(parts[0]!, 10);
    const secondOctet = parseInt(parts[1]!, 10);

    // 10.0.0.0/8
    if (firstOctet === 10) return true;

    // 172.16.0.0/12
    if (firstOctet === 172 && secondOctet >= 16 && secondOctet <= 31) return true;

    // 192.168.0.0/16
    if (firstOctet === 192 && secondOctet === 168) return true;
  }

  return false;
}
