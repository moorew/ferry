// tailcopy://capture, from the Control Center control and the Shortcuts action
// (SPEC 21.4): save the clipboard, say what happened, go back.

import { router } from 'expo-router';
import { useEffect } from 'react';

import { Core } from '../../modules/tailcopy-core';
import { t, type StringKey } from '../strings';
import { useToast } from '../ui/Toast';

const messages: Record<string, StringKey> = {
  saved: 'toast.saved',
  unchanged: 'toast.unchanged',
  empty: 'toast.empty',
  sensitive: 'toast.sensitive',
  denied: 'toast.denied',
};

export default function Capture() {
  const toast = useToast();
  useEffect(() => {
    Core.captureClipboard()
      .catch(() => 'denied' as const)
      .then((result) => {
        toast(t(messages[result]));
        router.replace('/');
      });
  }, [toast]);
  return null;
}
