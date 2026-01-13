import { useState } from 'react';
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
import { Loader2, Trash2, Copy, Check } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { Device } from 'shared/types';

// Generate random MAC address
function generateMACAddress(): string {
  const hex = () => Math.floor(Math.random() * 256).toString(16).padStart(2, '0');
  return `${hex()}:${hex()}:${hex()}:${hex()}:${hex()}:${hex()}`.toUpperCase();
}

// Simplified request type
interface RegisterDeviceRequest {
  device_name: string;
}

// Simplified response type
interface DeviceRegisterResponse {
  device_id: string;
  access_url: string;
  tunnel_id: string;
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
          <div className="space-y-4">
            {devices?.map((device) => (
              <Card key={device.id}>
                <CardContent className="pt-6">
                  <div className="flex justify-between items-start gap-4">
                    <div className="space-y-3 flex-1">
                      <div className="flex items-center gap-2">
                        <h4 className="font-medium">{device.name}</h4>
                        <Badge
                          variant={device.status === 'online' ? 'default' : 'secondary'}
                          className={
                            device.status === 'online'
                              ? 'bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/30 dark:text-green-400'
                              : ''
                          }
                        >
                          {device.status === 'online' ? t('tunnels:devices.status.online') : t('tunnels:devices.status.offline')}
                        </Badge>
                      </div>
                      <div className="space-y-2">
                        <Label className="text-xs">{t('tunnels:devices.accessUrl')}</Label>
                        <div className="flex gap-2">
                          <code className="flex-1 bg-muted px-3 py-2 rounded text-sm overflow-hidden text-ellipsis">
                            {window.location.origin}/api/tunnels/device?t={device.tunnel_id}
                          </code>
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => copyToClipboard(device.tunnel_id, device.id)}
                          >
                            {copiedId === device.id ? (
                              <Check className="h-4 w-4" />
                            ) : (
                              <Copy className="h-4 w-4" />
                            )}
                          </Button>
                        </div>
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => {
                        setDeviceToDelete(device);
                        setDeleteDialogOpen(true);
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}

            {devices?.length === 0 && (
              <div className="text-center py-12 text-muted-foreground">
                {t('tunnels:devices.none')}
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

  const register = useMutation({
    mutationFn: (data: RegisterDeviceRequest) => tunnelsApi.registerDevice(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tunnels', 'devices'] });
      onOpenChange(false);
      setName('');
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    register.mutate({ device_name: name });
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

const tunnelsApi = {
  listDevices: async (): Promise<Device[]> => {
    const response = await fetch('/api/tunnels/devices');
    if (!response.ok) {
      throw new Error(`Failed to fetch devices: ${response.statusText}`);
    }
    const result = await response.json();
    return result.data;
  },

  registerDevice: async (data: RegisterDeviceRequest): Promise<DeviceRegisterResponse> => {
    // Generate MAC address on client side
    const payload = {
      mac_address: generateMACAddress(),
      device_name: data.device_name,
    };
    const response = await fetch('/api/tunnels/devices', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      throw new Error(`Failed to register device: ${response.statusText}`);
    }
    return await response.json();
  },

  deleteDevice: async (deviceId: string) => {
    const response = await fetch(`/api/tunnels/devices/${deviceId}`, {
      method: 'DELETE',
    });
    if (!response.ok) {
      throw new Error(`Failed to delete device: ${response.statusText}`);
    }
  },
};
