<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { getAuthorizeClientInfo, submitConsent } from './api'
import type { ClientInfoResponse } from './api/type'
import UiButton from '@/components/ui/button/button.vue'
import { emitToast } from '@/utils/toast'

const route = useRoute()
const loading = ref(true)
const submitting = ref(false)
const clientInfo = ref<ClientInfoResponse | null>(null)
const error = ref('')

// Read authorize params from query
const authorizeParams = {
  client_id: (route.query.client_id as string) || '',
  redirect_uri: (route.query.redirect_uri as string) || '',
  response_type: (route.query.response_type as string) || 'code',
  code_challenge: (route.query.code_challenge as string) || '',
  code_challenge_method: (route.query.code_challenge_method as string) || 'S256',
  scope: (route.query.scope as string) || undefined,
  state: (route.query.state as string) || undefined,
  nonce: (route.query.nonce as string) || undefined,
}

onMounted(async () => {
  if (!authorizeParams.client_id) {
    error.value = 'Missing client_id parameter'
    loading.value = false
    return
  }

  try {
    const info = await getAuthorizeClientInfo({
      client_id: authorizeParams.client_id,
      scope: authorizeParams.scope,
    })
    clientInfo.value = info

    // If already consented, auto-approve
    if (info.already_consented) {
      await handleApprove()
      return
    }
  } catch (e) {
    console.error('Failed to load client info', e)
    error.value = 'Failed to load application information'
  } finally {
    loading.value = false
  }
})

async function handleApprove() {
  submitting.value = true
  try {
    const resp = await submitConsent({
      ...authorizeParams,
      approved: true,
    })
    // Redirect to the client's callback URL
    window.location.href = resp.redirect_url
  } catch (e) {
    console.error('Authorization failed', e)
    emitToast({ type: 'error', message: 'Authorization failed' })
    submitting.value = false
  }
}

async function handleDeny() {
  submitting.value = true
  try {
    const resp = await submitConsent({
      ...authorizeParams,
      approved: false,
    })
    window.location.href = resp.redirect_url
  } catch (e) {
    console.error('Deny failed', e)
    // Fallback: redirect with error
    const url = new URL(authorizeParams.redirect_uri)
    url.searchParams.set('error', 'access_denied')
    if (authorizeParams.state) {
      url.searchParams.set('state', authorizeParams.state)
    }
    window.location.href = url.toString()
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-muted p-4">
    <!-- Loading -->
    <div v-if="loading" class="flex items-center justify-center py-20">
      <span class="h-8 w-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></span>
    </div>

    <!-- Error -->
    <div v-else-if="error" class="w-full max-w-md bg-card text-card-foreground rounded-lg shadow-xl p-8 text-center">
      <svg class="w-16 h-16 text-destructive mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
      </svg>
      <h2 class="text-lg font-semibold text-foreground mb-2">Authorization Error</h2>
      <p class="text-muted-foreground">{{ error }}</p>
    </div>

    <!-- Consent Form -->
    <div v-else-if="clientInfo" class="w-full max-w-md bg-card text-card-foreground rounded-lg shadow-xl overflow-hidden">
      <!-- Header -->
      <div class="bg-gradient-to-r from-primary to-secondary p-6 text-white text-center">
        <div class="w-16 h-16 rounded-lg bg-white bg-opacity-20 flex items-center justify-center mx-auto mb-3">
          <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
          </svg>
        </div>
        <h1 class="text-xl font-bold">Authorize {{ clientInfo.client_name }}</h1>
      </div>

      <div class="p-6 space-y-6">
        <!-- App info -->
        <div class="text-center">
          <p class="text-sm text-muted-foreground">
            <strong class="text-foreground">{{ clientInfo.client_name }}</strong>
            wants to access your account
          </p>
        </div>

        <!-- Permissions -->
        <div>
          <h3 class="text-sm font-semibold text-foreground mb-3">This will allow the application to:</h3>
          <div class="space-y-2">
            <div
              v-for="scope in clientInfo.scopes"
              :key="scope.name"
              class="flex items-start gap-3 p-3 bg-muted rounded-lg"
            >
              <svg class="w-5 h-5 text-primary mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
              </svg>
              <div>
                <div class="text-sm font-medium text-foreground">{{ scope.name }}</div>
                <div v-if="scope.description" class="text-xs text-muted-foreground mt-0.5">
                  {{ scope.description }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Buttons -->
        <div class="flex gap-3">
          <UiButton
            class="flex-1"
            :disabled="submitting"
            @click="handleApprove"
          >
            <span v-if="submitting" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            Authorize {{ clientInfo.client_name }}
          </UiButton>
          <UiButton
            variant="outline"
            class="flex-1"
            :disabled="submitting"
            @click="handleDeny"
          >
            Cancel
          </UiButton>
        </div>

        <!-- Privacy note -->
        <p class="text-xs text-muted-foreground text-center">
          Authorizing will redirect to
          <code class="bg-muted px-1 py-0.5 rounded text-xs break-all">{{ authorizeParams.redirect_uri }}</code>
        </p>
      </div>
    </div>
  </div>
</template>
