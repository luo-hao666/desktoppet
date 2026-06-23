import { defineStore } from 'pinia'
import { ref } from 'vue'

export const usePetStore = defineStore('pet', () => {
  const currentState = ref('appear')
  const previousState = ref('appear')
  const petSize = ref(128)

  function switchState(newState, prevState) {
    previousState.value = prevState || currentState.value
    currentState.value = newState
  }

  return {
    currentState,
    previousState,
    petSize,
    switchState,
  }
})
