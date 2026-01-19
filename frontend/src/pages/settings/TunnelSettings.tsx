import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/hooks/auth/useAuth';
import { toast } from 'sonner';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Loader2, Trash2, Copy, Check, Power, PowerOff, Server, Cpu, Globe } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { MergedDevice, DeviceSource, RegisterDeviceResponse } from 'shared/types';
import { makeRequest, handleApiResponse } from '@/lib/api';

// Simplified request type matching Rust RegisterDeviceRequest
interface RegisterDeviceRequest {
  device_name: string;
  service_port?: number;
}

// Helper function to get display text for device source
function getDeviceSourceDisplay(source: DeviceSource): { label: string; className: string } {
  switch (source) {
    case 'Local':
      return {
        label: 'Local',
        className: 'bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/30 dark:text-blue-400',
      };
    case 'Gateway':
      return {
        label: 'Gateway',
        className: 'bg-purple-100 text-purple-700 hover:bg-purple-200 dark:bg-purple-900/30 dark:text-purple-400',
      };
    case 'Merged':
      return {
        label: 'Merged',
        className: 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400',
      };
    default:
      return {
        label: 'Unknown',
        className: 'bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-900/30 dark:text-gray-400',
      };
  }
}

export function TunnelSettings() {
  const { t } = useTranslation(['tunnels', 'common']);
  const { isSignedIn, login } = useAuth();
  const queryClient = useQueryClient();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [registerDialogOpen, setRegisterDialogOpen] = useState(false);
  const [loginRequiredDialogOpen, setLoginRequiredDialogOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deviceToDelete, setDeviceToDelete] = useState<MergedDevice | null>(null);

  const { data: devices, isLoading, error } = useQuery<MergedDevice[]>({
    queryKey: ['tunnels', 'devices'],
    queryFn: () => tunnelsApi.listDevices(),
    refetchInterval: 30000,
    enabled: isSignedIn, // Only fetch devices when user is logged in
  });

  const deleteDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.deleteDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      setDeleteDialogOpen(false);
      setDeviceToDelete(null);
      toast.success(t('tunnels:delete.success'));
    },
    onError: (error: Error) => {
      toast.error(t('tunnels:delete.error', { message: error.message }));
    },
  });

  const stopDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.stopDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      toast.success(t('tunnels:stop.success'));
    },
    onError: (error: Error) => {
      toast.error(t('tunnels:stop.error', { message: error.message }));
    },
  });

  const startDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.startDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      toast.success(t('tunnels:start.success'));
    },
    onError: (error: Error) => {
      toast.error(t('tunnels:start.error', { message: error.message }));
    },
  });

  const copyToClipboard = async (text: string, id: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const handleDeleteDevice = () => {
    if (deviceToDelete) {
      deleteDevice.mutate(deviceToDelete.id);
    }
  };

  const handleRegisterClick = () => {
    if (!isSignedIn) {
      setLoginRequiredDialogOpen(true);
    } else {
      setRegisterDialogOpen(true);
    }
  };

  const handleStopDevice = (deviceId: string) => {
    stopDevice.mutate(deviceId);
  };

  const handleStartDevice = (deviceId: string) => {
    startDevice.mutate(deviceId);
  };

  // Automatic heartbeat mechanism for running devices
  const heartbeatIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const isSignedInRef = useRef(isSignedIn);

  // Keep ref in sync with isSignedIn
  useEffect(() => {
    isSignedInRef.current = isSignedIn;
  }, [isSignedIn]);

  useEffect(() => {
    // Send heartbeat for each running device
    const sendHeartbeats = async () => {
      const runningDevices = devices?.filter(
        (device) => device.gost_process_id !== null
      ) || [];

      for (const device of runningDevices) {
        try {
          await tunnelsApi.heartbeat(device.id);
        } catch (error) {
          // Silently fail - heartbeat failures are logged by backend
          console.debug(`Heartbeat failed for device ${device.id}:`, error);
        }
      }
    };

    // Send initial heartbeat
    sendHeartbeats();

    // Set up interval (30 seconds, well under the 90-second timeout)
    heartbeatIntervalRef.current = setInterval(() => {
      sendHeartbeats();
    }, 30000);

    // Cleanup on unmount
    return () => {
      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
      }
    };
  }, [devices]);

  // Stop all running tunnels when user logs out
  useEffect(() => {
    if (!isSignedIn && devices && devices.length > 0) {
      const runningDevices = devices.filter(
        (device) => device.gost_process_id !== null
      );

      // Stop all running devices
      runningDevices.forEach((device) => {
        tunnelsApi.stopDevice(device.id).catch((error) => {
          console.debug(`Failed to stop device ${device.id} on logout:`, error);
        });
      });

      // Clear the devices data
      queryClient.setQueryData(['tunnels', 'devices'], []);
    }
  }, [isSignedIn, devices, queryClient]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8 gap-2">
        <Loader2 className="h-5 w-5 animate-spin" />
        <span>{t('common:loading')}</span>
      </div>
    );
  }

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertDescription>
          {error instanceof Error ? error.message : String(error)}
        </AlertDescription>
      </Alert>
    );
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <div className="flex justify-between items-center">
            <div>
              <CardTitle>{t('tunnels:title')}</CardTitle>
              <CardDescription>{t('tunnels:description')}</CardDescription>
            </div>
            <Button onClick={handleRegisterClick}>
              {t('common:buttons.register')}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            {devices?.map((device) => (
              <div
                key={device.id}
                className="group relative rounded-lg border bg-card p-3 sm:p-4 hover:bg-accent/50 transition-colors"
              >
                {/* Mobile: Vertical layout, Desktop: Horizontal layout */}
                <div className="flex flex-col sm:flex-row sm:items-center gap-3 sm:gap-4">
                  {/* Device Icon & Status */}
                  <div className="flex-shrink-0 flex items-center gap-3">
                    <div className={`h-10 w-10 sm:h-12 sm:w-12 rounded-lg flex items-center justify-center ${
                      device.gost_process_id !== null
                        ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                        : 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400'
                    }`}>
                      <Server className="h-5 w-5 sm:h-6 sm:w-6" />
                    </div>
                  </div>

                  {/* Device Info */}
                  <div className="flex-1 min-w-0 space-y-1.5 sm:space-y-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <h4 className="font-semibold text-sm sm:text-base truncate">{device.name}</h4>
                      <Badge
                        variant={device.status === 'online' ? 'default' : 'secondary'}
                        className={
                          device.status === 'online'
                            ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400 text-xs'
                            : 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400 text-xs'
                        }
                      >
                        <div className="h-1.5 w-1.5 sm:h-2 sm:w-2 rounded-full mr-1 bg-current" />
                        {device.status === 'online' ? t('tunnels:devices.status.online') : t('tunnels:devices.status.offline')}
                      </Badge>
                      <Badge
                        variant="outline"
                        className="text-xs"
                      >
                        <Globe className="h-2.5 w-2.5 sm:h-3 sm:w-3 mr-1" />
                        {Number(device.service_port) || 23001}
                      </Badge>
                      <Badge
                        variant="outline"
                        className={`text-xs ${getDeviceSourceDisplay(device.source).className}`}
                      >
                        {getDeviceSourceDisplay(device.source).label}
                      </Badge>
                    </div>

                    <div className="flex flex-wrap items-center gap-3 text-xs sm:text-sm text-muted-foreground">
                      <div className="flex items-center gap-1 min-w-0 flex-1 sm:flex-none">
                        <Cpu className="h-3.5 w-3.5 sm:h-4 sm:w-4 flex-shrink-0" />
                        <span className="font-mono text-xs truncate">{device.id}</span>
                      </div>
                      {device.gost_process_id !== null && (
                        <div className="flex items-center gap-1">
                          <Power className="h-3.5 w-3.5 sm:h-4 sm:w-4 text-green-600 dark:text-green-400" />
                          <span className="text-xs">PID: {Number(device.gost_process_id)}</span>
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-end gap-1 sm:gap-2 flex-shrink-0">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 sm:h-8 sm:w-8"
                      onClick={() => copyToClipboard(device.id, device.id)}
                      title={t('tunnels:devices.deviceId')}
                      aria-label={t('tunnels:devices.deviceId')}
                    >
                      {copiedId === device.id ? (
                        <Check className="h-4 w-4 text-green-600 dark:text-green-400" />
                      ) : (
                        <Copy className="h-4 w-4" />
                      )}
                    </Button>

                    {device.gost_process_id !== null ? (
                      <Button
                        variant="outline"
                        size="icon"
                        className="h-8 w-8 sm:h-8 sm:px-3 sm:w-auto"
                        onClick={() => handleStopDevice(device.id)}
                        disabled={stopDevice.isPending}
                        aria-label={t('tunnels:stopLabel')}
                      >
                        {stopDevice.isPending ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <>
                            <PowerOff className="h-4 w-4 sm:mr-1.5" />
                            <span className="hidden sm:inline">{t('tunnels:stopLabel')}</span>
                          </>
                        )}
                      </Button>
                    ) : (
                      <Button
                        variant="default"
                        size="icon"
                        className="h-8 w-8 sm:h-8 sm:px-3 sm:w-auto"
                        onClick={() => handleStartDevice(device.id)}
                        disabled={startDevice.isPending}
                        aria-label={t('tunnels:startLabel')}
                      >
                        {startDevice.isPending ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <>
                            <Power className="h-4 w-4 sm:mr-1.5" />
                            <span className="hidden sm:inline">{t('tunnels:startLabel')}</span>
                          </>
                        )}
                      </Button>
                    )}

                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 sm:h-8 sm:w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                      onClick={() => {
                        setDeviceToDelete(device);
                        setDeleteDialogOpen(true);
                      }}
                      title={t('common:buttons.delete')}
                      aria-label={t('common:buttons.delete')}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              </div>
            ))}

            {devices?.length === 0 && (
              <div className="text-center py-16">
                <Server className="h-16 w-16 mx-auto mb-4 text-muted-foreground/50" />
                <p className="text-muted-foreground">{t('tunnels:devices.none')}</p>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <RegisterDeviceDialog open={registerDialogOpen} onOpenChange={setRegisterDialogOpen} />

      <Dialog open={loginRequiredDialogOpen} onOpenChange={setLoginRequiredDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('common:oauth.loginRequired')}</DialogTitle>
            <DialogDescription>{t('common:oauth.loginRequiredMessage')}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setLoginRequiredDialogOpen(false)}>
              {t('common:buttons.cancel')}
            </Button>
            <Button onClick={login}>
              {t('common:oauth.loginWithCasdoor')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('tunnels:delete.title')}</DialogTitle>
            <DialogDescription>
              {t('tunnels:delete.description', { name: deviceToDelete?.name })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setDeleteDialogOpen(false);
                setDeviceToDelete(null);
              }}
            >
              {t('common:buttons.cancel')}
            </Button>
            <Button
              variant="destructive"
              onClick={handleDeleteDevice}
              disabled={deleteDevice.isPending}
            >
              {deleteDevice.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('common:buttons.delete')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function RegisterDeviceDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation(['tunnels', 'common']);
  const queryClient = useQueryClient();
  const [name, setName] = useState('');
  const [servicePort, setServicePort] = useState<string>('');

  const register = useMutation({
    mutationFn: (data: RegisterDeviceRequest) => tunnelsApi.registerDevice(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      onOpenChange(false);
      setName('');
      setServicePort('');
      toast.success(t('tunnels:register.success'));
    },
    onError: (error: Error) => {
      toast.error(t('tunnels:register.error', { message: error.message }));
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    register.mutate({
      device_name: name,
      service_port: servicePort ? parseInt(servicePort, 10) : undefined,
    });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>{t('tunnels:register.title')}</DialogTitle>
            <DialogDescription>{t('tunnels:register.description')}</DialogDescription>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="device-name">{t('tunnels:register.nameLabel')}</Label>
              <Input
                id="device-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('tunnels:register.namePlaceholder')}
                required
                autoFocus
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="service-port">{t('tunnels:register.portLabel')}</Label>
              <Input
                id="service-port"
                type="number"
                min="1"
                max="65535"
                value={servicePort}
                onChange={(e) => setServicePort(e.target.value)}
                placeholder={t('tunnels:register.portPlaceholder')}
              />
              <p className="text-xs text-muted-foreground">
                {t('tunnels:register.portHint')}
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('common:buttons.cancel')}
            </Button>
            <Button type="submit" disabled={register.isPending}>
              {register.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {t('common:buttons.register')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// Local API endpoints (vibe-kanban backend)
const tunnelsApi = {
  listDevices: async (): Promise<MergedDevice[]> => {
    const response = await makeRequest('/api/tunnels/devices');
    return handleApiResponse<MergedDevice[]>(response);
  },

  getDevice: async (deviceId: string): Promise<MergedDevice> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}`);
    return handleApiResponse<MergedDevice>(response);
  },

  registerDevice: async (data: RegisterDeviceRequest): Promise<RegisterDeviceResponse> => {
    const response = await makeRequest('/api/tunnels/devices', {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return handleApiResponse<RegisterDeviceResponse>(response);
  },

  deleteDevice: async (deviceId: string): Promise<void> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}`, {
      method: 'DELETE',
    });
    return handleApiResponse<void>(response);
  },

  startDevice: async (deviceId: string): Promise<void> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}/start`, {
      method: 'POST',
    });
    return handleApiResponse<void>(response);
  },

  stopDevice: async (deviceId: string): Promise<void> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}/stop`, {
      method: 'POST',
    });
    return handleApiResponse<void>(response);
  },

  heartbeat: async (deviceId: string): Promise<void> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}/heartbeat`, {
      method: 'POST',
    });
    return handleApiResponse<void>(response);
  },
};
