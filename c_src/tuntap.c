#include <errno.h>
#include <fcntl.h>
#include <linux/if_tun.h>
#include <net/if.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>

#ifdef __cplusplus
extern "C" {
#endif
int32_t setup_tun_device(int32_t fd, char const *ifname);

#ifdef __cplusplus
} // extern "C"
#endif

int32_t setup_device(int32_t fd, char const *ifname, int32_t flags) {
  struct ifreq ifr;
  memset(&ifr, 0, sizeof(ifr));

  ifr.ifr_flags = flags;
  memcpy(ifr.ifr_name, ifname, IFNAMSIZ);

  if (ioctl(fd, TUNSETIFF, (void *)&ifr) < 0) {
    return -1;
  }
  return 0;
}

int32_t setup_tun_device(int32_t fd, char const *ifname) {
  return setup_device(fd, ifname, IFF_TUN | IFF_NO_PI);
}
