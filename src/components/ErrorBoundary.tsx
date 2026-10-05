import React, { ErrorInfo, ReactNode } from 'react';
import { AlertTriangle } from 'lucide-react';
import { t } from '../i18n/index.tsx';

interface Props {
  children?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends React.Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error:', error, errorInfo);
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="fixed inset-0 overflow-hidden flex flex-col items-center justify-center p-4 bg-background text-foreground">
          <AlertTriangle className="w-16 h-16 text-red-500 mb-6" />
          <h1 className="text-2xl font-bold mb-3 uppercase tracking-normal leading-none">
            {t('errors.critical')}
          </h1>
          <p className="text-muted-foreground mb-6 text-center max-w-md font-medium">
            {t('errors.description')}
          </p>
          {this.state.error && (
            <pre className="bg-surface/50 p-4 rounded-xl text-[0.7rem] sm:text-xs font-mono w-full max-w-2xl overflow-auto border border-border/50 shadow-inner mb-6 max-h-[200px] text-red-400">
              {this.state.error.message}
              {'\n\n'}
              {this.state.error.stack}
            </pre>
          )}
          <button
            onClick={() => {
              const url = new URL(window.location.href);
              url.searchParams.set('retry', Date.now().toString());
              window.location.href = url.toString();
            }}
            className="bg-primary hover:bg-primary-hover text-primary-foreground px-8 py-3 rounded-lg font-bold transition-all shadow-lg uppercase tracking-tight text-sm active:scale-95"
          >
            {t('errors.reload')}
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
