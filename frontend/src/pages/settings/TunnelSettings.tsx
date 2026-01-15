import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuth } from '@/hooks/auth/useAuth';
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
import type { Device, RegisterDeviceResponse } from 'shared/types';
import { makeRequest, handleApiResponse } from '@/lib/api';

// Simplified request type matching Rust RegisterDeviceRequest
interface RegisterDeviceRequest {
  device_name: string;
  service_port?: number;
}

export function TunnelSettings() {
  const { t } = useTranslation(['tunnels', 'common']);
  const { isSignedIn, login } = useAuth();
  const queryClient = useQueryClient();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [registerDialogOpen, setRegisterDialogOpen] = useState(false);
  const [loginRequiredDialogOpen, setLoginRequiredDialogOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deviceToDelete, setDeviceToDelete] = useState<Device | null>(null);

  const { data: devices, isLoading, error } = useQuery<Device[]>({
    queryKey: ['tunnels', 'devices'],
    queryFn: () => tunnelsApi.listDevices(),
    refetchInterval: 30000,
  });

  const deleteDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.deleteDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      setDeleteDialogOpen(false);
      setDeviceToDelete(null);
    },
  });

  const stopDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.stopDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
    },
  });

  const startDevice = useMutation({
    mutationFn: (deviceId: string) => tunnelsApi.startDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
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
                className="group relative rounded-lg border bg-card p-4 hover:bg-accent/50 transition-colors"
              >
                <div className="flex items-center gap-4">
                  {/* Device Icon & Status */}
                  <div className="flex-shrink-0">
                    <div className={`h-12 w-12 rounded-lg flex items-center justify-center ${
                      device.gost_process_id !== null
                        ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                        : 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400'
                    }`}>
                      <Server className="h-6 w-6" />
                    </div>
                  </div>

                  {/* Device Info */}
                  <div className="flex-1 min-w-0 space-y-2">
                    <div className="flex items-center gap-3">
                      <h4 className="font-semibold text-base truncate">{device.name}</h4>
                      <Badge
                        variant={device.status === 'online' ? 'default' : 'secondary'}
                        className={
                          device.status === 'online'
                            ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400'
                            : 'bg-gray-100 text-gray-700 dark:bg-gray-900/30 dark:text-gray-400'
                        }
                      >
                        <div className="h-2 w-2 rounded-full mr-1.5 bg-current" />
                        {device.status === 'online' ? t('tunnels:devices.status.online') : t('tunnels:devices.status.offline')}
                      </Badge>
                      <Badge
                        variant="outline"
                        className="text-xs"
                      >
                        <Globe className="h-3 w-3 mr-1" />
                        {Number(device.service_port) || 23001}
                      </Badge>
                    </div>

                    <div className="flex items-center gap-4 text-sm text-muted-foreground">
                      <div className="flex items-center gap-1.5 min-w-0">
                        <Cpu className="h-4 w-4 flex-shrink-0" />
                        <span className="font-mono text-xs truncate">{device.id}</span>
                      </div>
                      {device.gost_process_id !== null && (
                        <div className="flex items-center gap-1.5">
                          <Power className="h-4 w-4 text-green-600 dark:text-green-400" />
                          <span className="text-xs">PID: {Number(device.gost_process_id)}</span>
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 flex-shrink-0">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8"
                      onClick={() => copyToClipboard(device.id, device.id)}
                      title={t('tunnels:devices.deviceId')}
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
                        size="sm"
                        className="h-8 px-3"
                        onClick={() => handleStopDevice(device.id)}
                        disabled={stopDevice.isPending}
                      >
                        {stopDevice.isPending ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <>
                            <PowerOff className="h-4 w-4 mr-1.5" />
                            {t('tunnels:stop')}
                          </>
                        )}
                      </Button>
                    ) : (
                      <Button
                        variant="default"
                        size="sm"
                        className="h-8 px-3"
                        onClick={() => handleStartDevice(device.id)}
                        disabled={startDevice.isPending}
                      >
                        {startDevice.isPending ? (
                          <Loader2 className="h-4 w-4 animate-spin" />
                        ) : (
                          <>
                            <Power className="h-4 w-4 mr-1.5" />
                            {t('tunnels:start')}
                          </>
                        )}
                      </Button>
                    )}

                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-destructive hover:text-destructive hover:bg-destructive/10"
                      onClick={() => {
                        setDeviceToDelete(device);
                        setDeleteDialogOpen(true);
                      }}
                      title={t('common:buttons.delete')}
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
  listDevices: async (): Promise<Device[]> => {
    const response = await makeRequest('/api/tunnels/devices');
    return handleApiResponse<Device[]>(response);
  },

  getDevice: async (deviceId: string): Promise<Device> => {
    const response = await makeRequest(`/api/tunnels/devices/${deviceId}`);
    return handleApiResponse<Device>(response);
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
